//! Every core command a client can send to a running chat. The shapes that a
//! command carries (permission answers, question replies, form replies, modes,
//! efforts) are iterated from their lists, so each of their variants is sent.

use yaca_core::process::ProcessId;
use yaca_core::Command;

use crate::ids;
use crate::variants;

pub fn all() -> Vec<Command> {
    let mut commands = vec![
        Command::SendMessage {
            text: "Make greet() say hello to the world.".to_owned(),
            attachments: vec![],
        },
        Command::SendMessage {
            text: "Check this file too.".to_owned(),
            attachments: vec!["src/greet.rs".to_owned()],
        },
        Command::Cancel,
        Command::PreviewCheckpoint {
            id: ids::checkpoint(1),
        },
        Command::RestoreCheckpoint {
            id: ids::checkpoint(1),
        },
        Command::Compact,
        Command::EditSummary {
            text: "The greeting now says hello, world.".to_owned(),
        },
        Command::SetBypass { on: true },
        Command::SetBypass { on: false },
        Command::WatchProcess {
            id: Some(ProcessId(3)),
        },
        Command::WatchProcess { id: None },
        Command::StopProcess { id: ProcessId(3) },
        Command::ReadFileView {
            path: "src/greet.rs".to_owned(),
        },
        Command::CancelQueued {
            id: ids::queued(1),
        },
        Command::SearchFiles {
            query: "gre".to_owned(),
        },
        Command::SetModel {
            model: "model-0".to_owned(),
        },
        Command::ReconnectMcpServer {
            server: "filesystem".to_owned(),
        },
        Command::ApproveMcpServer {
            server: "notes".to_owned(),
        },
        Command::LoginMcpServer {
            server: "asana".to_owned(),
        },
        Command::LogoutMcpServer {
            server: "asana".to_owned(),
        },
        Command::RestartLspServer {
            server: "rust-analyzer".to_owned(),
        },
    ];

    commands.extend(
        variants::modes()
            .into_iter()
            .map(|mode| Command::SetMode { mode }),
    );
    commands.extend(
        variants::efforts()
            .into_iter()
            .map(|effort| Command::SetEffort { effort }),
    );
    commands.extend(
        variants::permission_answers()
            .into_iter()
            .map(|answer| Command::AnswerPermission {
                request_id: ids::request(1),
                answer,
            }),
    );
    commands.extend(
        variants::question_replies()
            .into_iter()
            .map(|answer| Command::AnswerQuestion {
                request_id: ids::request(2),
                answer,
            }),
    );
    commands.extend(
        variants::elicitation_replies()
            .into_iter()
            .map(|reply| Command::AnswerElicitation {
                request_id: ids::request(3),
                reply,
            }),
    );
    commands
}
