use crate::{
    ActionRequest, ActionResult, AdapterError, ref_action::into_adapter_error,
    ref_action::ref_action_context::RefActionContext,
    ref_action::ref_action_wait_context::RefActionWaitContext,
};

pub(crate) fn execute_single_shot(
    context: RefActionWaitContext<'_>,
    request: ActionRequest,
    deadline: crate::Deadline,
    lease: &crate::InteractionLease,
    dispatch: impl Fn(
        RefActionContext<'_>,
        ActionRequest,
        &crate::InteractionLease,
    ) -> Result<ActionResult, crate::AppError>,
) -> Result<ActionResult, AdapterError> {
    let keyboard_is_aimed_at_the_element = request.action.delivered_by_keyboard()
        && !context.adapter.keyboard_needs_window_activation();
    if request.headed_requirement().requires_focus() && !keyboard_is_aimed_at_the_element {
        let requirement = if matches!(request.action, crate::Action::Click) {
            context
                .adapter
                .headed_requirement_for_ref(context.entry, &request, deadline)
                .map_err(crate::ref_action::mark_pre_dispatch_resolution_failure)?
        } else {
            request.headed_requirement()
        };
        if requirement == crate::HeadedRequirement::FocusedApplication {
            let instance = context
                .entry
                .process
                .process_instance
                .as_deref()
                .filter(|instance| !instance.is_empty())
                .ok_or_else(|| {
                    AdapterError::stale_ref_because("Target process identity is unavailable")
                })?;
            context.adapter.focus_app(
                crate::ProcessIdentity::new(context.entry.process.pid, instance),
                lease,
            )?;
        } else if requirement.requires_focus() {
            crate::actions::headed_focus::focus_entry_window(
                context.entry,
                context.adapter,
                context.context,
                lease,
            )
            .map_err(crate::ref_action::into_adapter_error)?;
        }
    }
    dispatch(RefActionContext::new(context, deadline), request, lease).map_err(into_adapter_error)
}
