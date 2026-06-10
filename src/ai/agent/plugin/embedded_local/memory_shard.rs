use std::path::PathBuf;

use async_trait::async_trait;

use crate::ai::agent::IAgentPlugin;
use crate::ai::agent::interceptor::DynInterceptor;
use crate::ai::agent::interceptor::IInterceptor;
use crate::ai::agent::interceptor::InterceptorFlow;
use crate::ai::agent::tool::DynTool;
use crate::ai::agent::tool::embedded_local::memory::{
    GenerateMemoryShardTool, ModifyMemoryShardTool, RecallMemoryShardTool, parse_frontmatter,
};
use crate::ai::resolver::IResolver;
use crate::ai::resolver::context::AnnotatedMessage;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver::message::MessageRef;

// ---------------------------------------------------------------------------
// MemoryShardPlugin
// ---------------------------------------------------------------------------

pub fn memory_shard_plugin(memory_dir: PathBuf) -> MemoryShardPlugin {
    MemoryShardPlugin::new(memory_dir)
}

/// Plugin that provides all memory-shard functionality: directory injection
/// (interceptor) and CRUD operations (tools).
pub struct MemoryShardPlugin {
    memory_dir: PathBuf,
}

impl MemoryShardPlugin {
    pub fn new(memory_dir: PathBuf) -> Self {
        Self { memory_dir }
    }
}

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for MemoryShardPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    fn tools(&mut self) -> Vec<DynTool> {
        vec![
            Box::new(RecallMemoryShardTool::new(self.memory_dir.clone())),
            Box::new(GenerateMemoryShardTool::new(self.memory_dir.clone())),
            Box::new(ModifyMemoryShardTool::new(self.memory_dir.clone())),
        ]
    }

    fn interceptors(&mut self) -> Vec<DynInterceptor<S, M, A>> {
        vec![Box::new(MemoryShardInterceptor::<M, S, A>::new(
            self.memory_dir.clone(),
        ))]
    }
}

// ---------------------------------------------------------------------------
// MemoryShardInterceptor
// ---------------------------------------------------------------------------

struct MemoryShardInterceptor<M, S, A> {
    shards_dir: PathBuf,
    #[allow(clippy::complexity)]
    marker: std::marker::PhantomData<fn() -> (M, S, A)>,
}

impl<M, S, A> MemoryShardInterceptor<M, S, A> {
    fn new(memory_dir: PathBuf) -> Self {
        Self {
            shards_dir: memory_dir.join("shards"),
            marker: std::marker::PhantomData,
        }
    }

    fn collect_shards(&self) -> String {
        let dir = match std::fs::read_dir(&self.shards_dir) {
            Ok(dir) => dir,
            Err(_) => return String::new(),
        };

        let mut shards: Vec<String> = Vec::new();

        for entry in dir {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let shard_file = path.join("shard.md");
            if !shard_file.exists() {
                continue;
            }

            let raw = match std::fs::read_to_string(&shard_file) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let meta = match parse_frontmatter(&raw) {
                Ok((m, _)) => m,
                Err(_) => continue,
            };

            shards.push(format!(
                "- 名称：{}\n  描述：{}\n  标签：{}",
                meta.name,
                meta.description,
                meta.tags.join(", ")
            ));
        }

        if shards.is_empty() {
            String::new()
        } else {
            shards.join("\n")
        }
    }
}

#[async_trait]
impl<M, S, A> IInterceptor<S, M, A> for MemoryShardInterceptor<M, S, A>
where
    M: IMessage + Send + Sync + 'static,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    async fn before_evaluate(&mut self, _state: &mut S, cx: &mut Context<M, A>) -> InterceptorFlow {
        if last_user_text(cx).is_none() {
            return InterceptorFlow::Continue;
        }

        let shards = self.collect_shards();
        if shards.is_empty() {
            return InterceptorFlow::Continue;
        }

        let prefix = "[注入上下文：记忆分片目录]\n\
            来源：系统\n\
            用途：可用记忆分片目录\n\
            说明：这不是真实用户发言。不要直接回应本消息。需要完整内容时，按名称调用 recall_memory_shard。\n\n";
        let content = format!("{}{}\n[/注入上下文]", prefix, shards);

        let injected = M::from(MessageOwned::User { content });
        cx.inject_before_last(AnnotatedMessage::new(injected, A::default()));

        InterceptorFlow::Continue
    }
}

fn last_user_text<M, A>(cx: &Context<M, A>) -> Option<&str>
where
    M: IMessage + Send + Sync + 'static,
{
    let message = cx.annotated_messages().last()?.message.message_ref();
    match message {
        MessageRef::User { content } => Some(content),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use openai_oxide::types::chat::ChatCompletionMessageParam;

    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::message::MessageRef;

    #[tokio::test]
    async fn injects_shard_list_before_user_message() {
        let mut interceptor = MemoryShardInterceptor::<ChatCompletionMessageParam, (), ()>::new(
            PathBuf::from("memory"),
        );
        let mut state = ();
        let mut cx =
            ContextBuilder::<ChatCompletionMessageParam>::new("test-model")
                .messages(vec![MessageRef::User {
                    content: "[channel_id: 1, channel_name: -, sender_id: 2, sender_nickname: 小明, sender_channel_nickname: -, sender_prks_id: -, sent_at: now]\nhello",
                }
                .into()])
                .build();

        interceptor.before_evaluate(&mut state, &mut cx).await;

        assert!(
            cx.message_count() >= 2,
            "应该在用户消息前注入至少一条记忆分片目录消息"
        );

        let injected_idx = cx.message_count() - 2;
        match cx.annotated_messages()[injected_idx].message.message_ref() {
            MessageRef::User { content } => {
                assert!(
                    content.starts_with("[注入上下文：记忆分片目录]"),
                    "注入消息应该是记忆分片目录，实际为：{}",
                    content
                );
            }
            other => panic!("注入消息应该是用户消息类型，实际为：{:?}", other),
        }

        match cx
            .annotated_messages()
            .last()
            .unwrap()
            .message
            .message_ref()
        {
            MessageRef::User { content } => {
                assert!(content.contains("hello"), "最后一条消息应该是原始用户消息");
            }
            other => panic!("最后一条消息应该是用户消息类型，实际为：{:?}", other),
        }
    }

    #[tokio::test]
    async fn skips_when_no_user_message() {
        let mut interceptor = MemoryShardInterceptor::<ChatCompletionMessageParam, (), ()>::new(
            PathBuf::from("memory"),
        );
        let mut state = ();
        let mut cx = ContextBuilder::<ChatCompletionMessageParam>::new("test-model").build();

        interceptor.before_evaluate(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 0, "上下文为空时不应该注入消息");
    }
}
