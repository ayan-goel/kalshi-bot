use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BotState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
    Switching,
}

impl std::fmt::Display for BotState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BotState::Stopped => write!(f, "stopped"),
            BotState::Starting => write!(f, "starting"),
            BotState::Running => write!(f, "running"),
            BotState::Stopping => write!(f, "stopping"),
            BotState::Error => write!(f, "error"),
            BotState::Switching => write!(f, "switching"),
        }
    }
}

impl BotState {
    pub fn can_transition_to(&self, target: BotState) -> bool {
        matches!(
            (self, target),
            (BotState::Stopped, BotState::Starting)
                | (BotState::Starting, BotState::Running)
                | (BotState::Starting, BotState::Error)
                | (BotState::Running, BotState::Stopping)
                | (BotState::Running, BotState::Error)
                // Bug 17: allow Running -> Stopped for the Stop/Kill command path,
                // which calls transition(Stopped) after the trading task has already
                // exited (possibly without passing through Stopping).
                | (BotState::Running, BotState::Stopped)
                | (BotState::Running, BotState::Switching)
                | (BotState::Stopping, BotState::Stopped)
                | (BotState::Error, BotState::Stopped)
                | (BotState::Error, BotState::Starting)
                | (BotState::Switching, BotState::Stopped)
                | (BotState::Switching, BotState::Error)
        )
    }
}

pub struct BotStateMachine {
    state: BotState,
    db_pool: PgPool,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    error_message: Option<String>,
}

impl BotStateMachine {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            state: BotState::Stopped,
            db_pool,
            started_at: None,
            error_message: None,
        }
    }

    pub fn state(&self) -> BotState {
        self.state
    }

    pub fn started_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.started_at
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn is_running(&self) -> bool {
        self.state == BotState::Running
    }

    pub async fn transition(
        &mut self,
        target: BotState,
        trigger: &str,
        details: Option<serde_json::Value>,
    ) -> Result<(), String> {
        if !self.state.can_transition_to(target) {
            return Err(format!(
                "Invalid transition from {} to {}",
                self.state, target
            ));
        }

        let from = self.state;
        self.state = target;

        match target {
            BotState::Running => {
                self.started_at = Some(chrono::Utc::now());
                self.error_message = None;
            }
            BotState::Stopped => {
                self.started_at = None;
                self.error_message = None;
            }
            BotState::Error => {
                self.error_message = details
                    .as_ref()
                    .and_then(|d| d.get("message"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| Some(trigger.to_string()));
            }
            _ => {}
        }

        info!(from = %from, to = %target, trigger = %trigger, "Bot state transition");

        let _ = self.persist_transition(from, target, trigger, details).await;
        Ok(())
    }

    async fn persist_transition(
        &self,
        from: BotState,
        to: BotState,
        trigger: &str,
        details: Option<serde_json::Value>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO bot_state_history (ts, from_state, to_state, trigger, details)
            VALUES (NOW(), $1, $2, $3, $4)
            "#,
        )
        .bind(from.to_string())
        .bind(to.to_string())
        .bind(trigger)
        .bind(details)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }
}
