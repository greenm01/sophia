use crate::prelude::*;

pub fn real_atomic_runtime_rendered_scanout_renderer_observation() -> LiveRendererRuntimeObservation
{
    LiveRendererRuntimeObservation::from_startup_status(
        LiveRendererImportBoundary::with_native_imports(false, true).startup_status(),
        LiveRendererSelectionObservation::NativeImportCapable,
    )
}
