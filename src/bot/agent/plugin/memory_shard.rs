use std::path::PathBuf;

use async_trait::async_trait;

use crate::ai::agent::AgentBuilder;
use crate::ai::agent::IAgentPlugin;
use crate::ai::agent::interceptor::IInterceptor;
use crate::ai::agent::interceptor::InterceptorFlow;
use crate::ai::agent::tool::local::memory::parse_frontmatter;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::context::AnnotatedMessage;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver::message::MessageRef;
use crate::bot::agent::memory_dir;

pub fn plugin_memory_shard() -> MemoryShardPlugin {
    MemoryShardPlugin
}

pub struct MemoryShardPlugin;

impl<M, R, S, A> IAgentPlugin<M, R, S, A> for MemoryShardPlugin
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send,
    S: Send + Sync + 'static,
    A: Default + Send + Sync + 'static,
{
    fn apply(&self, builder: AgentBuilder<M, R, S, A>) -> AgentBuilder<M, R, S, A> {
        builder.interceptor(MemoryShardInterceptor::<M, S, A>::new(memory_dir()))
    }
}

struct MemoryShardInterceptor<M, S, A> {
    shards_dir: PathBuf,
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
                "- **{}**: {} [tags: {}]",
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
    async fn before_solve(
        &mut self,
        _state: &mut S,
        cx: &mut Context<M, A>,
    ) -> InterceptorFlow {
        let Some(_user_text) = latest_user_text(cx) else {
            return InterceptorFlow::Continue;
        };

        let shards = self.collect_shards();
        if shards.is_empty() {
            return InterceptorFlow::Continue;
        }

        let prefix = "[Available memory shards]\n\
            You have access to the following memory shards. \
            Use recall_memory_shard to read a shard's full content by name:\n\n";
        let content = format!("{}{}", prefix, shards);

        inject_before_latest_message(cx, content);
        InterceptorFlow::Continue
    }
}

fn latest_user_text<M, A>(cx: &Context<M, A>) -> Option<&str>
where
    M: IMessage + Send + Sync + 'static,
{
    let message = cx.annotated_messages().last()?.message.message_ref();
    match message {
        MessageRef::User { content } => Some(content),
        _ => None,
    }
}

fn inject_before_latest_message<M, A>(cx: &mut Context<M, A>, content: String)
where
    M: IMessage + Send + Sync + 'static,
    A: Default,
{
    let mut messages = cx.take_annotated_messages();
    let latest = match messages.pop() {
        Some(msg) => msg,
        None => {
            cx.set_annotated_messages(messages);
            return;
        }
    };

    let injected = M::from(MessageOwned::User {
        content,
    });
    let annotation = A::default();
    messages.push(AnnotatedMessage::new(injected, annotation));
    messages.push(latest);
    cx.set_annotated_messages(messages);
}

#[cfg(test)]
mod tests {
    use super::*;

    use openai_oxide::types::chat::ChatCompletionMessageParam;

    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::message::MessageRef;

    #[tokio::test]
    async fn injects_shard_list_before_user_message() {
        let mut interceptor =
            MemoryShardInterceptor::<ChatCompletionMessageParam, (), ()>::new(
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

        interceptor.before_solve(&mut state, &mut cx).await;

        assert!(cx.message_count() >= 2, "should inject at least one shard message before user message");

        // The injected message should be at second-to-last position, user message at last.
        let injected_idx = cx.message_count() - 2;
        match cx.annotated_messages()[injected_idx].message.message_ref() {
            MessageRef::User { content } => {
                assert!(
                    content.starts_with("[Available memory shards]"),
                    "injected message should be the shard list, got: {content}"
                );
            }
            other => panic!("injected message should be User, got: {:?}", other),
        }

        // The last message should still be the original user message.
        match cx.annotated_messages().last().unwrap().message.message_ref() {
            MessageRef::User { content } => {
                assert!(
                    content.contains("hello"),
                    "last message should be the original user message"
                );
            }
            other => panic!("last message should be User, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn skips_when_no_user_message() {
        let mut interceptor =
            MemoryShardInterceptor::<ChatCompletionMessageParam, (), ()>::new(
                PathBuf::from("memory"),
            );
        let mut state = ();
        let mut cx =
            ContextBuilder::<ChatCompletionMessageParam>::new("test-model").build();

        interceptor.before_solve(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 0, "should not inject when context is empty");
    }
}
