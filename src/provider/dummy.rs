use std::{future::Future, pin::Pin};

use anyhow::{Result, bail};
use tokio::sync::mpsc::UnboundedReceiver;

use super::{Message, Provider, StreamEvent, ToolDefinition};

pub struct DummyProvider {
    model: String,
}

impl DummyProvider {
    pub fn new() -> Self {
        Self {
            model: "setup-required".to_string(),
        }
    }
}

impl Default for DummyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for DummyProvider {
    fn name(&self) -> &str {
        "setup"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: String) {
        self.model = model;
    }

    fn available_models(&self) -> Vec<String> {
        vec![self.model.clone()]
    }

    fn context_window(&self) -> u32 {
        0
    }

    fn fetch_context_window(&self) -> Pin<Box<dyn Future<Output = Result<u32>> + Send + '_>> {
        Box::pin(async { Ok(0) })
    }

    fn fetch_models(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async { Ok(vec!["setup-required".to_string()]) })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
        _max_tokens: u32,
        _thinking_budget: u32,
    ) -> Pin<Box<dyn Future<Output = Result<UnboundedReceiver<StreamEvent>>> + Send + '_>> {
        Box::pin(async {
            bail!(
                "No credentials configured. Set ANTHROPIC_API_KEY/OPENAI_API_KEY or run `dot login`."
            )
        })
    }
}
