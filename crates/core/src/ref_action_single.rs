use crate::{
    ActionRequest, ActionResult, AdapterError, ref_action::into_adapter_error,
    ref_action_context::RefActionContext, ref_action_wait_context::RefActionWaitContext,
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
    if request.headed_requirement().requires_focus() {
        crate::headed_focus::focus_entry_window(
            context.entry,
            context.adapter,
            context.context,
            lease,
        )
        .map_err(crate::ref_action::into_adapter_error)?;
    }
    dispatch(RefActionContext::new(context, deadline), request, lease).map_err(into_adapter_error)
}
