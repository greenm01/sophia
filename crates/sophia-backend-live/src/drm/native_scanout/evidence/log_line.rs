use crate::prelude::*;

impl LibdrmNativeAtomicScanoutSmokeEvidence {
    pub fn reduced_log_line(&self) -> String {
        fn status<T: std::fmt::Debug>(status: Option<T>) -> String {
            status
                .map(|status| format!("{status:?}"))
                .unwrap_or_else(|| "none".to_owned())
        }

        let (commit_page_flip_event, commit_nonblocking, commit_allow_modeset, commit_test_only) =
            self.commit_flags
                .map(|flags| {
                    (
                        flags.page_flip_event.to_string(),
                        flags.nonblocking.to_string(),
                        flags.allow_modeset.to_string(),
                        flags.test_only.to_string(),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        "none".to_owned(),
                        "none".to_owned(),
                        "none".to_owned(),
                        "none".to_owned(),
                    )
                });

        format!(
            "sophia_atomic_scanout_evidence schema=8 phase={:?} status={:?} scanout_target={} rendered_context={} gbm_export={} gbm_export_detail={} scanout_buffer={} properties={} resources={} framebuffer={} request={} submit={} request_scope={} commit_page_flip_event={} commit_nonblocking={} commit_allow_modeset={} commit_test_only={} page_flip_wait={} page_flip_poll={} page_flip={} retire={} retire_destroy={} retire_cleanup_pending={}",
            self.phase,
            self.status,
            status(self.scanout_target),
            status(self.rendered_context),
            status(self.gbm_export),
            status(self.gbm_export_detail),
            status(self.scanout_buffer),
            status(self.properties),
            status(self.resources),
            status(self.framebuffer),
            status(self.request),
            status(self.submit),
            status(self.request_scope),
            commit_page_flip_event,
            commit_nonblocking,
            commit_allow_modeset,
            commit_test_only,
            status(self.page_flip_wait),
            status(self.page_flip_poll),
            status(self.page_flip),
            status(self.retire),
            status(self.retire_destroy),
            self.retire_cleanup_pending,
        )
    }
}
