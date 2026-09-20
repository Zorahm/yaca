//! Режим сессии и то, что из него следует (§7.1, §8).
//!
//! Человек переключает одну ось — [`Mode`], четыре взаимоисключающих пункта в
//! меню разрешений. [`Intent`] и [`TrustMode`] из него выводятся и хранимым
//! состоянием не являются: ядро, умеющее комбинации, которых в окне не
//! выбрать, — это вторая версия правды, и она разъедется на первом же
//! переключении.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Режим сессии. Единственное, что переключает человек (§8).
///
/// План стоит первым: список идёт от осторожного к доверчивому, а план
/// осторожнее ручного — записывающих тулов в нём нет вовсе, тогда как ручной
/// про них спрашивает.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Читать и планировать. Записывающих тулов модель не получает вовсе.
    Plan,
    /// Ничего без спроса. Дефолт — Yaca работает и со слабыми локальными моделями.
    #[default]
    Manual,
    /// Всё кроме выполнения команд.
    Auto,
    /// Применять правки, спрашивать остальное.
    Edits,
}

impl Mode {
    /// Какие тулы вообще попадают в схему запроса.
    pub fn intent(self) -> Intent {
        match self {
            Mode::Plan => Intent::Plan,
            Mode::Manual | Mode::Auto | Mode::Edits => Intent::Agent,
        }
    }

    /// По какой строке матрицы §8 решается вопрос о разрешении.
    ///
    /// Своей строки у плана нет, а ответ нужен: записывающих тулов там не
    /// бывает, но `web_fetch` бывает во всех режимах, и разрешение он
    /// спрашивает. Берётся `Manual` — план это разведка, и вопрос про сеть в
    /// ней уместен. Помнить доверие, выбранное до плана, значило бы держать
    /// под чипом состояние, которого на чипе не видно.
    pub fn trust(self) -> TrustMode {
        match self {
            Mode::Plan | Mode::Manual => TrustMode::Manual,
            Mode::Auto => TrustMode::Auto,
            Mode::Edits => TrustMode::Edits,
        }
    }
}

/// Ось намерения: какие тулы попадают в схему запроса. Выводится из [`Mode`].
///
/// §7.1: «Записывающие тулы не отдаются модели вообще: слабая модель проигнорирует
/// запрет в промпте и потратит ход на ошибку.» Поэтому это фильтр реестра,
/// а не строчка в системном промпте.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Intent {
    /// Читающие плюс `todo_write` и `exit_plan_mode`.
    Plan,
    /// Полный набор.
    #[default]
    Agent,
}

impl Intent {
    pub fn allows(self, allowed: &[Intent]) -> bool {
        allowed.contains(&self)
    }
}

/// Строка матрицы доверия §8. Выводится из [`Mode`] и наружу не уезжает:
/// интерфейс переключает режим, а не эту ось отдельно.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TrustMode {
    /// Ничего без спроса.
    #[default]
    Manual,
    /// Всё кроме выполнения команд.
    Auto,
    /// Применять правки, спрашивать остальное.
    Edits,
}

/// Уровень усилия рассуждения (§7.1, правый нижний угол композера).
///
/// Ложится на `output_config.effort` Messages API один в один. Фиксированный
/// бюджет токенов (`thinking.budget_tokens`) здесь не при чём: на Opus 5,
/// Sonnet 5 и Opus 4.7+ он удалён и отдаёт 400.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    /// «Extra» из §7.1. Лучший уровень для кодовых и агентных задач.
    #[default]
    Extra,
    Max,
}

impl Effort {
    /// Значение для `output_config.effort`.
    pub fn as_api(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Extra => "xhigh",
            Effort::Max => "max",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::policy::{Policy, Verdict};
    use crate::permission::PermissionKind;

    fn policy(mode: Mode) -> Policy {
        Policy {
            trust: mode.trust(),
            ..Policy::default()
        }
    }

    #[test]
    fn только_план_снимает_с_модели_пишущие_тулы() {
        assert_eq!(Mode::Plan.intent(), Intent::Plan);
        for mode in [Mode::Manual, Mode::Auto, Mode::Edits] {
            assert_eq!(mode.intent(), Intent::Agent, "{mode:?}");
        }
    }

    #[test]
    fn план_спрашивает_про_сеть() {
        // Записывающих тулов в плане нет, но `web_fetch` есть, и строка §8 ему
        // нужна. Взята `Ручной`: план это разведка, и вопрос про сеть уместен.
        let reach = PermissionKind::FetchUrl;
        assert_eq!(policy(Mode::Plan).verdict(reach), Verdict::Ask);
        assert_eq!(policy(Mode::Manual).verdict(reach), Verdict::Ask);
        // А «Авто» про сеть не спрашивает: план ведёт себя как «Ручной».
        assert_eq!(policy(Mode::Auto).verdict(reach), Verdict::Allow);
    }

    #[test]
    fn план_ведёт_себя_как_ручной_и_в_вопросе_о_правке() {
        // Записывающих тулов в плане нет, но если бы дошло до вопроса —
        // ответ тот же, что у «Ручного»: спросить.
        assert_eq!(
            policy(Mode::Plan).verdict(PermissionKind::EditFile),
            Verdict::Ask
        );
        assert_eq!(
            policy(Mode::Manual).verdict(PermissionKind::EditFile),
            Verdict::Ask
        );
        assert_eq!(
            policy(Mode::Auto).verdict(PermissionKind::EditFile),
            Verdict::Allow
        );
    }
}
