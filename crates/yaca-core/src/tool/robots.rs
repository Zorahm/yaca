//! `robots.txt`: чей запрет Yaca соблюдает и как он попадает в отказ тула.
//!
//! Свой разбор вместо зависимости — по той же причине, по которой в окне свой
//! i18n: нужны группы, `Allow`/`Disallow` и два джокера, а не дерево крейтов.
//! Правила ниже — RFC 9309, и отклонения от него помечены там, где сделаны.
//!
//! **Сверяется продуктовый токен, а не вся строка `User-Agent`.** То есть
//! `YacaTraveler` из [`super::net`], без учёта регистра; своей группы нет —
//! берётся `*`. Отсюда и требование к имени: оно обязано быть таким, чтобы
//! правило о нём не задело ничего другого.
//!
//! **Победа самого длинного совпадения.** Путь может попасть и под `Allow`, и
//! под `Disallow`; выигрывает тот образец, что длиннее, а при равной длине —
//! `Allow`. Это RFC 9309 §2.2.2, и оно же единственная тонкость, ради которой
//! правила нельзя просто перебрать по порядку.
//!
//! **Процентное кодирование сверяется как есть.** RFC просит сначала привести
//! путь и образец к одному виду; здесь сравниваются те байты, что пришли, —
//! `url.path()` уже закодирован, и правила обычно пишут так же. Расходится это
//! на редкой экзотике вроде `Disallow: /пример` против `/%D0%BF...`, и честнее
//! назвать границу здесь, чем чинить её вслепую.
//!
//! **Недоступность — не запрет.** RFC велит считать неотвеченный `robots.txt`
//! полным запретом. Для краулера это правильно, для Yaca нет: человек только
//! что руками разрешил этот домен (§5.7), и упавший на секунду сервер закрыл
//! бы страницу по причине, которой никто не видит. Поэтому ошибка сети и 5xx
//! разрешают, как и 404. Запрещает только прочитанный файл, в котором запрет
//! написан.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use reqwest::Url;

use super::net;
use super::ToolCtx;
use crate::error::ToolError;

/// Сколько разобранный файл живёт в памяти процесса.
///
/// Без кэша каждая страница — два запроса, и `robots.txt` одного хоста
/// вытягивался бы по десятку раз за ход. Час — верхняя граница, которую RFC
/// называет разумной; сессии короче, так что на деле файл читается один раз.
const TTL: Duration = Duration::from_secs(60 * 60);

/// Потолок на сам файл. RFC просит разбирать хотя бы 500 КиБ; всё, что
/// больше, — уже не правила, а чья-то ошибка, и тянуть её в память незачем.
const MAX_BYTES: usize = 512 * 1024;

/// Сколько редиректов `robots.txt` проходит. Клиент их не следует сам
/// ([`net::client`]), а RFC просит пройти хотя бы пять.
const MAX_REDIRECTS: usize = 5;

/// Одно правило: образец пути и что он значит.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    pattern: String,
    allow: bool,
}

/// Разобранный файл — только та группа, что относится к нам.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rules {
    rules: Vec<Rule>,
}

impl Rules {
    /// Пустые правила: файла нет, он не ответил или в нём нет нашей группы.
    pub fn allow_all() -> Self {
        Rules::default()
    }

    /// Разбирает файл для продуктового токена `agent`.
    ///
    /// Групп в файле много, и склеивать их нельзя: правила чужой группы к нам
    /// не относятся. Берётся своя, а не нашлась — `*`.
    pub fn parse(text: &str, agent: &str) -> Self {
        let agent = agent.to_ascii_lowercase();
        let mut mine: Option<Vec<Rule>> = None;
        let mut fallback: Option<Vec<Rule>> = None;

        // Идущие подряд `User-agent` — одна группа на всех. Флаги говорят,
        // кому достаются правила, которые пойдут следом.
        let mut for_me = false;
        let mut for_all = false;
        // Правило после группы закрывает её: следующий `User-agent` начинает
        // новую, а не дописывает в эту.
        let mut collecting = false;

        for line in text.lines() {
            let Some((field, value)) = split(line) else {
                continue;
            };

            match field.as_str() {
                "user-agent" => {
                    if collecting {
                        for_me = false;
                        for_all = false;
                        collecting = false;
                    }
                    let value = value.to_ascii_lowercase();
                    if value == agent {
                        for_me = true;
                    } else if value == "*" {
                        for_all = true;
                    }
                }
                "allow" | "disallow" => {
                    collecting = true;
                    let allow = field == "allow";
                    // Пустой `Disallow` — это «ничего не запрещено», то есть
                    // отсутствие правила, а не запрет пустого пути.
                    if !allow && value.is_empty() {
                        continue;
                    }
                    let rule = Rule {
                        pattern: value.to_string(),
                        allow,
                    };
                    if for_me {
                        mine.get_or_insert_with(Vec::new).push(rule.clone());
                    }
                    if for_all {
                        fallback.get_or_insert_with(Vec::new).push(rule);
                    }
                }
                // `Crawl-delay`, `Sitemap` и прочее Yaca не читает: она не
                // обходит сайт, а берёт одну страницу по просьбе человека.
                _ => {}
            }
        }

        Rules {
            rules: mine.or(fallback).unwrap_or_default(),
        }
    }

    /// Правило, решающее судьбу пути: самый длинный подошедший образец, при
    /// равной длине — разрешающий (RFC 9309 §2.2.2).
    ///
    /// Одно место на оба вопроса — «пустят ли» и «что именно не пустило»:
    /// два прохода с чуть разным выбором лучшего расходятся на первой же
    /// правке.
    fn best(&self, path: &str) -> Option<&Rule> {
        self.rules
            .iter()
            .filter(|rule| matches(&rule.pattern, path))
            .fold(None, |best: Option<&Rule>, rule| match best {
                Some(other) if other.pattern.len() > rule.pattern.len() => Some(other),
                Some(other) if other.pattern.len() == rule.pattern.len() && other.allow => {
                    Some(other)
                }
                _ => Some(rule),
            })
    }

    /// Образец, который не пускает, — или `None`, если путь разрешён.
    ///
    /// Один вопрос вместо двух: «пустят ли» без «чем именно не пустило» тут
    /// никому не нужен — отказ обязан назвать правило, иначе человеку нечего
    /// чинить, а модели нечего сказать.
    fn blocking(&self, path: &str) -> Option<&str> {
        self.best(path)
            .filter(|rule| !rule.allow)
            .map(|rule| rule.pattern.as_str())
    }
}

/// `field: value` с обрезанным комментарием. Строка не той формы пропускается.
fn split(line: &str) -> Option<(String, &str)> {
    let line = line.split('#').next().unwrap_or("").trim();
    let (field, value) = line.split_once(':')?;
    let field = field.trim().to_ascii_lowercase();
    if field.is_empty() {
        return None;
    }
    Some((field, value.trim()))
}

/// Совпадение образца с путём: `*` — любая последовательность, `$` в конце —
/// конец строки, остальное буквально.
///
/// Поиск в лоб с возвратом: образцы в `robots.txt` короткие, и городить
/// автомат ради десятка строк было бы работой не по задаче.
fn matches(pattern: &str, path: &str) -> bool {
    let anchored = pattern.ends_with('$');
    let pattern = if anchored {
        &pattern[..pattern.len() - 1]
    } else {
        pattern
    };

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut at = 0usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        // Первый кусок обязан стоять в начале: образец без ведущего `*`
        // привязан к началу пути.
        if index == 0 {
            if !path[at..].starts_with(part) {
                return false;
            }
            at += part.len();
            continue;
        }
        let Some(found) = path[at..].find(part) else {
            return false;
        };
        at += found + part.len();
    }

    if !anchored {
        return true;
    }
    // С `$` последний кусок обязан упереться в конец — если он не `*`.
    match parts.last() {
        Some(last) if !last.is_empty() => at == path.len(),
        _ => true,
    }
}

/// Кэш по origin. Ключ — `scheme://host:port`: у `robots.txt` своя область на
/// каждую схему и порт, и правила `http://` к `https://` не относятся.
type Cache = Mutex<HashMap<String, (Rules, Instant)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn origin_of(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// Проверяет адрес и, если он запрещён, возвращает отказ.
///
/// [`ToolError::Rejected`], а не `Failed`: ничего не сломалось — сработала
/// граница, ровно как у редиректа на чужой хост. Модель получает объяснение с
/// образцом, который её остановил, и может попросить другое.
pub async fn check(ctx: &ToolCtx, url: &Url) -> Result<(), ToolError> {
    let rules = rules_for(ctx, url).await;
    let path = path_of(url);

    if let Some(pattern) = rules.blocking(&path) {
        return Err(ToolError::Rejected(format!(
            "robots.txt на {} запрещает этот путь правилом `Disallow: {pattern}`. \
             Yaca такие запреты соблюдает; выключается в настройках, раздел «Инструменты».",
            url.host_str().unwrap_or_default()
        )));
    }
    Ok(())
}

/// Путь вместе со строкой запроса: правила `robots.txt` пишутся и по ней.
fn path_of(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    }
}

/// Правила хоста: из кэша или с сервера.
async fn rules_for(ctx: &ToolCtx, url: &Url) -> Rules {
    let origin = origin_of(url);

    if let Some((rules, at)) = cache().lock().unwrap().get(&origin) {
        if at.elapsed() < TTL {
            return rules.clone();
        }
    }

    let rules = fetch(ctx, url).await;
    cache()
        .lock()
        .unwrap()
        .insert(origin, (rules.clone(), Instant::now()));
    rules
}

/// Тянет и разбирает `robots.txt`. Любая неудача — разрешение: см. шапку.
async fn fetch(ctx: &ToolCtx, url: &Url) -> Rules {
    let Ok(mut at) = url.join("/robots.txt") else {
        return Rules::allow_all();
    };

    for _ in 0..=MAX_REDIRECTS {
        if ctx.cancel.is_cancelled() {
            return Rules::allow_all();
        }

        let Ok(response) = net::client().get(at.clone()).send().await else {
            return Rules::allow_all();
        };

        if response.status().is_redirection() {
            let next = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| at.join(v).ok());
            match next {
                Some(next) => {
                    at = next;
                    continue;
                }
                None => return Rules::allow_all(),
            }
        }

        // 404 и родня — правил нет. 5xx — сервер не ответил, и по причинам из
        // шапки это тоже разрешение, а не запрет.
        if !response.status().is_success() {
            return Rules::allow_all();
        }

        let Ok(body) = response.bytes().await else {
            return Rules::allow_all();
        };
        let text = String::from_utf8_lossy(&body[..body.len().min(MAX_BYTES)]);
        return Rules::parse(&text, net::PRODUCT_TOKEN);
    }

    Rules::allow_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(text: &str) -> Rules {
        Rules::parse(text, "yacatraveler")
    }

    /// Пускают ли. Обёртка над [`Rules::blocking`]: тесты про правила, а не
    /// про то, как выглядит их ответ.
    fn allows(rules: &Rules, path: &str) -> bool {
        rules.blocking(path).is_none()
    }

    #[test]
    fn an_empty_file_allows_everything() {
        assert!(allows(&rules(""), "/anything"));
    }

    #[test]
    fn a_star_group_applies_when_we_are_not_named() {
        let r = rules("User-agent: *\nDisallow: /private\n");
        assert!(!allows(&r, "/private/page"));
        assert!(allows(&r, "/public"));
    }

    /// Своя группа перебивает `*` целиком, а не дополняет её: правила чужой
    /// группы к нам не относятся, и `*` — тоже чужая, когда есть своя.
    #[test]
    fn our_own_group_replaces_the_star_group() {
        let r = rules("User-agent: *\nDisallow: /\n\nUser-agent: YacaTraveler\nDisallow: /secret\n");
        assert!(allows(&r, "/anything"), "запрет из `*` к нам не относится");
        assert!(!allows(&r, "/secret"));
    }

    #[test]
    fn the_name_is_matched_without_case() {
        let r = Rules::parse("User-agent: YACATRAVELER\nDisallow: /x\n", "yacatraveler");
        assert!(!allows(&r, "/x"));
    }

    /// Идущие подряд `User-agent` делят одну группу правил.
    #[test]
    fn agents_listed_together_share_their_rules() {
        let r = rules("User-agent: SomeBot\nUser-agent: YacaTraveler\nDisallow: /both\n");
        assert!(!allows(&r, "/both"));
    }

    /// Правило после группы закрывает её: следующий `User-agent` начинает
    /// новую, и чужой запрет к нам не прилипает.
    #[test]
    fn a_new_group_starts_after_the_rules_of_the_previous_one() {
        let r = rules("User-agent: YacaTraveler\nDisallow: /mine\nUser-agent: Other\nDisallow: /theirs\n");
        assert!(!allows(&r, "/mine"));
        assert!(allows(&r, "/theirs"));
    }

    /// Самый длинный образец побеждает — иначе порядок строк решал бы, что
    /// значит файл (RFC 9309 §2.2.2).
    #[test]
    fn the_longest_pattern_wins_over_the_order_of_lines() {
        let r = rules("User-agent: *\nDisallow: /docs\nAllow: /docs/public\n");
        assert!(!allows(&r, "/docs/secret"));
        assert!(allows(&r, "/docs/public/page"));

        // И в обратном порядке строк — ответ тот же.
        let r = rules("User-agent: *\nAllow: /docs/public\nDisallow: /docs\n");
        assert!(allows(&r, "/docs/public/page"));
    }

    #[test]
    fn equal_length_goes_to_allow() {
        let r = rules("User-agent: *\nDisallow: /a\nAllow: /a\n");
        assert!(allows(&r, "/a"));
    }

    #[test]
    fn an_empty_disallow_forbids_nothing() {
        let r = rules("User-agent: *\nDisallow:\n");
        assert!(allows(&r, "/anything"));
    }

    #[test]
    fn comments_and_odd_spacing_are_survived() {
        let r = rules("# заметка\n  User-agent :  *  # кто\n Disallow : /x  \n");
        assert!(!allows(&r, "/x"));
    }

    #[test]
    fn a_star_stands_for_anything() {
        let r = rules("User-agent: *\nDisallow: /*/private\n");
        assert!(!allows(&r, "/a/private"));
        assert!(!allows(&r, "/a/b/private"));
        assert!(allows(&r, "/private"));
    }

    #[test]
    fn a_dollar_pins_the_end() {
        let r = rules("User-agent: *\nDisallow: /*.pdf$\n");
        assert!(!allows(&r, "/paper.pdf"));
        assert!(allows(&r, "/paper.pdf.html"));
    }

    /// Запрет пишется и по строке запроса, поэтому путь сверяется вместе с ней.
    #[test]
    fn the_query_string_is_part_of_the_path() {
        let r = rules("User-agent: *\nDisallow: /search?q=\n");
        assert!(!allows(&r, "/search?q=secret"));
        assert!(allows(&r, "/search"));
    }

    #[test]
    fn the_blocking_pattern_is_reported_for_the_explanation() {
        let r = rules("User-agent: *\nDisallow: /private\n");
        assert_eq!(r.blocking("/private/page"), Some("/private"));
        assert_eq!(r.blocking("/public"), None);
    }

    /// Разрешение — не «запретивший образец»: объяснять нечего.
    #[test]
    fn an_allowed_path_has_no_blocking_pattern() {
        let r = rules("User-agent: *\nDisallow: /docs\nAllow: /docs/ok\n");
        assert_eq!(r.blocking("/docs/ok"), None);
        assert_eq!(r.blocking("/docs/no"), Some("/docs"));
    }
}
