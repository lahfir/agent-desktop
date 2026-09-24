#pragma once

/// Outlines the exact target window while the cursor pose is shown.
///
/// Call only while the cue itself is presented for a visible target. Each call
/// re-reads the target's window-server record, so the outline follows moves and
/// resizes, and it stays invisible unless it is proven to sit immediately above
/// the target with no foreign window overlapping the target from above.
void ADGlowRefresh(void);

/// Orders the outline out. It stays hidden until the next `ADGlowRefresh`.
void ADGlowHide(void);

/// Sets the shared cue opacity for the outline. It never orders the outline
/// in; a hidden outline only records the value for its next placement.
void ADGlowSetOpacity(double alpha);
