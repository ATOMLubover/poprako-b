use rand::Rng;

use crate::ai::agent::interceptor::{IInterceptor, InterceptorFlow};
use crate::ai::resolver::context::Context;
use crate::ai::resolver::message::{IMessage, MessageRef};
use crate::ai::resolver_impl::deepseek::data_object::DeepSeekMessage;
use crate::bot::agent::plugin::review::annotation::{
    IReviewAnnotated, ReviewAnnotation, ReviewMessageSource,
};
use crate::bot::agent::plugin::review::decision::run_reviewer;
use crate::bot::agent::plugin::review::input::review_channel_id;
use crate::bot::agent::plugin::review::state::{IReviewEmbedded, SolveKind};

fn latest_user_annotation_source(kind: SolveKind) -> ReviewMessageSource {
    match kind {
        SolveKind::Normal => ReviewMessageSource::NormalUser,
        SolveKind::ReviewFollowup => ReviewMessageSource::ReviewFeedbackUser,
    }
}

fn final_assistant_annotation_source(kind: SolveKind) -> ReviewMessageSource {
    match kind {
        SolveKind::Normal => ReviewMessageSource::NormalAssistant,
        SolveKind::ReviewFollowup => ReviewMessageSource::ReviewFollowupAssistant,
    }
}

fn annotate_latest_user<M, A>(cx: &mut Context<M, A>, respond_id: &str, source: ReviewMessageSource)
where
    M: IMessage + Send + Sync + 'static,
    A: IReviewAnnotated + Send + Sync + 'static,
{
    let Some(message) = cx.annotated_messages_mut().last_mut() else {
        return;
    };

    if !matches!(message.message.message_ref(), MessageRef::User { .. }) {
        return;
    }

    *message.annotation.review_annotation_mut() =
        ReviewAnnotation::new(respond_id.to_string(), source);
}

fn annotate_latest_assistant<M, A>(
    cx: &mut Context<M, A>,
    respond_id: &str,
    source: ReviewMessageSource,
) where
    M: IMessage + Send + Sync + 'static,
    A: IReviewAnnotated + Send + Sync + 'static,
{
    for message in cx.annotated_messages_mut().iter_mut().rev() {
        if matches!(message.message.message_ref(), MessageRef::Assist { .. }) {
            *message.annotation.review_annotation_mut() =
                ReviewAnnotation::new(respond_id.to_string(), source);
            break;
        }
    }
}

pub struct ReviewInterceptor {
    sample: fn() -> bool,
}

impl ReviewInterceptor {
    pub fn new() -> Self {
        Self {
            // FIXME: Change to 50%.
            sample: || rand::thread_rng().gen_ratio(2, 2),
        }
    }

    #[cfg(test)]
    pub fn with_sample(sample: fn() -> bool) -> Self {
        Self { sample }
    }
}

impl Default for ReviewInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<S, A> IInterceptor<S, DeepSeekMessage, A> for ReviewInterceptor
where
    S: IReviewEmbedded + Send + Sync + 'static,
    A: IReviewAnnotated + Clone + Default + Send + Sync + 'static,
{
    async fn before_evaluate(
        &mut self,
        state: &mut S,
        cx: &mut Context<DeepSeekMessage, A>,
    ) -> InterceptorFlow {
        let runtime = state.review_runtime();
        let Some(respond_id) = runtime.respond_id().map(str::to_string) else {
            return InterceptorFlow::Continue;
        };

        annotate_latest_user(
            cx,
            &respond_id,
            latest_user_annotation_source(runtime.solve_kind()),
        );

        InterceptorFlow::Continue
    }

    async fn after_evaluate(
        &mut self,
        state: &mut S,
        cx: &mut Context<DeepSeekMessage, A>,
        output: &mut Option<String>,
    ) -> InterceptorFlow {
        let runtime = state.review_runtime();
        let solve_kind = runtime.solve_kind();
        let Some(respond_id) = runtime.respond_id().map(str::to_string) else {
            return InterceptorFlow::Continue;
        };

        annotate_latest_assistant(
            cx,
            &respond_id,
            final_assistant_annotation_source(solve_kind),
        );

        let should_review = solve_kind == SolveKind::Normal
            && output
                .as_deref()
                .map(str::trim)
                .is_some_and(|text| !text.is_empty())
            && (self.sample)();

        if should_review && let Some(event_send) = runtime.event_send() {
            let channel_id = review_channel_id(cx, &respond_id);
            let cloned = cx.clone();
            tokio::spawn(run_reviewer(cloned, channel_id, respond_id, event_send));
        }

        state.review_runtime_mut().clear_solve();
        InterceptorFlow::Continue
    }
}
