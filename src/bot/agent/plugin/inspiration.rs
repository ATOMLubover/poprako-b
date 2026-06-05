use std::collections::HashSet;

use async_trait::async_trait;

use crate::ai::agent::Agent;
use crate::ai::agent::AgentBuilder;
use crate::ai::agent::compact::Compact;
use crate::ai::agent::compact::SlidingWindowCompact;
use crate::ai::agent::interceptor::Interceptor;
use crate::ai::agent::interceptor::InterceptorFlow;
use crate::ai::resolver::IResolver;
use crate::ai::resolver::context::AnnotatedMessage;
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::IMessage;
use crate::ai::resolver::message::MessageOwned;
use crate::ai::resolver::message::MessageRef;

pub type InspiredAgent<M, R> = Agent<M, R, InspirationState, InspirationAnnotation>;

pub type InspiredAgentBuilder<M, R> = AgentBuilder<M, R, InspirationState, InspirationAnnotation>;

#[derive(Debug, Default)]
pub struct InspirationState {
    active_inspiration_ids: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InspirationAnnotation {
    inspiration_id: Option<String>,
}

impl InspirationAnnotation {
    fn inspiration(id: impl Into<String>) -> Self {
        Self {
            inspiration_id: Some(id.into()),
        }
    }
}

pub fn plugin_inspiration<M, R>(builder: InspiredAgentBuilder<M, R>) -> InspiredAgentBuilder<M, R>
where
    M: IMessage + Send + Sync + 'static,
    R: IResolver<Message = M> + Send + 'static,
{
    builder
        .compact(InspirationCompact::<M>::default())
        .interceptor(InspirationInterceptor::<M>::default())
}

struct InspirationInterceptor<M> {
    knowledge: InspirationKnowledge,
    marker: std::marker::PhantomData<fn() -> M>,
}

impl<M> Default for InspirationInterceptor<M> {
    fn default() -> Self {
        Self {
            knowledge: InspirationKnowledge,
            marker: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<M> Interceptor<InspirationState, M, InspirationAnnotation> for InspirationInterceptor<M>
where
    M: IMessage + Send + Sync + 'static,
{
    async fn before_solve(
        &mut self,
        state: &mut InspirationState,
        cx: &mut Context<M, InspirationAnnotation>,
    ) -> InterceptorFlow {
        let Some(user_text) = latest_user_text(cx) else {
            return InterceptorFlow::Continue;
        };

        let input = MatchInput::parse(user_text);
        let injections = self.knowledge.match_entries(&input, state);
        if injections.is_empty() {
            return InterceptorFlow::Continue;
        }

        inject_before_latest_message(cx, injections, state);
        InterceptorFlow::Continue
    }
}

struct InspirationCompact<M> {
    inner: SlidingWindowCompact<M, InspirationState, InspirationAnnotation>,
}

impl<M> Default for InspirationCompact<M> {
    fn default() -> Self {
        Self {
            inner: SlidingWindowCompact::default(),
        }
    }
}

#[async_trait]
impl<M> Compact for InspirationCompact<M>
where
    M: IMessage + Send + Sync + 'static,
{
    type Message = M;
    type State = InspirationState;
    type Annotation = InspirationAnnotation;

    async fn compact(
        &mut self,
        state: &mut InspirationState,
        cx: &mut Context<Self::Message, Self::Annotation>,
    ) {
        self.inner.compact(state, cx).await;
        state.active_inspiration_ids = retained_inspiration_ids(cx);
    }
}

#[derive(Debug, Clone, Copy)]
struct KnowledgeEntry {
    id: &'static str,
    keywords: &'static [&'static str],
    content: &'static str,
}

#[derive(Default)]
struct InspirationKnowledge;

impl InspirationKnowledge {
    fn match_entries(
        &self,
        input: &MatchInput<'_>,
        state: &InspirationState,
    ) -> Vec<KnowledgeEntry> {
        knowledge_entries()
            .iter()
            .copied()
            .filter(|entry| !state.active_inspiration_ids.contains(entry.id))
            .filter(|entry| entry.matches(input))
            .collect()
    }
}

impl KnowledgeEntry {
    fn matches(&self, input: &MatchInput<'_>) -> bool {
        self.keywords.iter().any(|keyword| input.contains(keyword))
    }
}

struct MatchInput<'a> {
    sender_nickname: Option<&'a str>,
    sender_group_nickname: Option<&'a str>,
    body: &'a str,
}

impl<'a> MatchInput<'a> {
    fn parse(prompt_text: &'a str) -> Self {
        let (meta, body) = prompt_text
            .split_once('\n')
            .unwrap_or((prompt_text, prompt_text));

        Self {
            sender_nickname: metadata_value(meta, "sender_nickname"),
            sender_group_nickname: metadata_value(meta, "sender_group_nickname"),
            body,
        }
    }

    fn contains(&self, keyword: &str) -> bool {
        self.body.contains(keyword)
            || self.sender_nickname == Some(keyword)
            || self.sender_group_nickname == Some(keyword)
    }
}

fn latest_user_text<M>(cx: &Context<M, InspirationAnnotation>) -> Option<&str>
where
    M: IMessage + Send + Sync + 'static,
{
    let message = cx.annotated_messages().last()?.message.message_ref();
    match message {
        MessageRef::User { content } => Some(content),
        _ => None,
    }
}

fn inject_before_latest_message<M>(
    cx: &mut Context<M, InspirationAnnotation>,
    injections: Vec<KnowledgeEntry>,
    state: &mut InspirationState,
) where
    M: IMessage + Send + Sync + 'static,
{
    let mut messages = cx.take_annotated_messages();
    let Some(latest) = messages.pop() else {
        cx.set_annotated_messages(messages);
        return;
    };

    for entry in injections {
        let content = format!("[灵光一闪]\n{}", entry.content);
        let message = M::from(MessageOwned::User { content });
        messages.push(AnnotatedMessage::new(
            message,
            InspirationAnnotation::inspiration(entry.id),
        ));
        state.active_inspiration_ids.insert(entry.id.to_string());
    }

    messages.push(latest);
    cx.set_annotated_messages(messages);
}

fn retained_inspiration_ids<M>(cx: &Context<M, InspirationAnnotation>) -> HashSet<String>
where
    M: IMessage + Send + Sync + 'static,
{
    cx.annotated_messages()
        .iter()
        .filter_map(|message| message.annotation.inspiration_id.clone())
        .collect()
}

fn metadata_value<'a>(meta: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{}: ", key);
    let start = meta.find(&prefix)? + prefix.len();
    let rest = &meta[start..];
    let end = rest.find(',').unwrap_or(rest.len());
    let value = rest[..end].trim();

    if value == "-" || value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn knowledge_entries() -> &'static [KnowledgeEntry] {
    &[
        KnowledgeEntry {
            id: "member.lb",
            keywords: &["LB"],
            content: "LB：核心开发，负责 poprako 全系列工具的开发",
        },
        KnowledgeEntry {
            id: "member.niuniu",
            keywords: &["牛牛", "灰暗天穹"],
            content: "牛牛 / 灰暗天穹：喜欢剧情、画工、萝莉；巨乳是减分项",
        },
        KnowledgeEntry {
            id: "member.nabai",
            keywords: &["那白"],
            content: "那白：翻译，热爱学习译法，喜欢用告白台词开玩笑",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    use openai_oxide::types::chat::ChatCompletionMessageParam;

    use crate::ai::resolver::context::ContextBuilder;
    use crate::ai::resolver::message::MessageRef;

    fn user(content: &str) -> ChatCompletionMessageParam {
        MessageRef::User { content }.into()
    }

    fn annotated_user(
        content: &str,
        annotation: InspirationAnnotation,
    ) -> AnnotatedMessage<ChatCompletionMessageParam, InspirationAnnotation> {
        AnnotatedMessage::new(user(content), annotation)
    }

    #[tokio::test]
    async fn before_solve_injects_matched_inspiration_before_latest_user_message() {
        let mut interceptor = InspirationInterceptor::default();
        let mut state = InspirationState::default();
        let mut cx = ContextBuilder::new("test-model")
            .messages(vec![user(
                "[group_qid: 1, group_name: -, sender_qid: 2, sender_nickname: LB, sender_group_nickname: -, sender_prks_id: -, sent_at: now]\n帮我看看",
            )])
            .build();

        interceptor.before_solve(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 2);
        assert!(state.active_inspiration_ids.contains("member.lb"));
        match cx.annotated_messages()[1].message.message_ref() {
            MessageRef::User { content } => assert_eq!(
                content,
                "[group_qid: 1, group_name: -, sender_qid: 2, sender_nickname: LB, sender_group_nickname: -, sender_prks_id: -, sent_at: now]\n帮我看看"
            ),
            _ => panic!("latest message should remain a user message"),
        }
        assert_eq!(
            cx.annotated_messages()[0]
                .annotation
                .inspiration_id
                .as_deref(),
            Some("member.lb")
        );
    }

    #[tokio::test]
    async fn before_solve_does_not_duplicate_active_inspiration() {
        let mut interceptor = InspirationInterceptor::default();
        let mut state = InspirationState::default();
        state
            .active_inspiration_ids
            .insert("member.nabai".to_string());
        let mut cx = ContextBuilder::new("test-model")
            .messages(vec![user(
                "[group_qid: 1, group_name: -, sender_qid: 2, sender_nickname: 小明, sender_group_nickname: -, sender_prks_id: -, sent_at: now]\n那白在吗",
            )])
            .build();

        interceptor.before_solve(&mut state, &mut cx).await;

        assert_eq!(cx.message_count(), 1);
    }

    #[tokio::test]
    async fn compact_rebuilds_state_from_retained_annotations() {
        let mut state = InspirationState::default();
        state.active_inspiration_ids.insert("member.lb".to_string());
        state
            .active_inspiration_ids
            .insert("member.nabai".to_string());
        let mut cx = ContextBuilder::new("test-model")
            .annotated_messages(vec![
                annotated_user(
                    "[灵光一闪]\n那白：翻译",
                    InspirationAnnotation::inspiration("member.nabai"),
                ),
                annotated_user("那白在吗", InspirationAnnotation::default()),
            ])
            .build();
        let mut compact = InspirationCompact::default();

        compact.compact(&mut state, &mut cx).await;

        assert_eq!(state.active_inspiration_ids.len(), 1);
        assert!(state.active_inspiration_ids.contains("member.nabai"));
    }
}
