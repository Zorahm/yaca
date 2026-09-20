//! Идентификаторы. Новотипы поверх строки: в Rust не перепутать, в TS — обычный `string`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
        #[ts(export)]
        pub struct $name(pub String);

        impl $name {
            /// Свежий идентификатор с читаемым префиксом — по логу видно, что это.
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Сессия — один разговор в проекте.
    SessionId,
    "ses"
);
string_id!(
    /// Турн — один заход агента от сообщения человека до остановки.
    TurnId,
    "turn"
);
string_id!(
    /// Блок текста или рассуждения внутри ответа модели.
    BlockId,
    "blk"
);
string_id!(
    /// Вызов тула. Совпадает с `tool_use.id` провайдера, когда тот его даёт.
    CallId,
    "call"
);
string_id!(
    /// Запрос разрешения (§5.7). В срезе 1 не используется, но живёт в контракте.
    RequestId,
    "req"
);
string_id!(
    /// Чекпоинт (§5.8).
    CheckpointId,
    "ckpt"
);

/// Миллисекунды эпохи. В TS — обычное `number`, укладывается в 2^53 до 287-го тысячелетия.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
