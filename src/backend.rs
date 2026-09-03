//! External backends for source input, or for supplying action values directly.
//!
//! A backend can provide input frames the same way a keyboard or a gamepad does, or it can bypass
//! this crate's own bindings and supply an action's value directly, the way a platform's own input
//! service (Steam Input, say) does. Either way, a context reading the action does not need to know
//! which one produced the result.
