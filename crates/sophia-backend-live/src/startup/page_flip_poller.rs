use crate::prelude::*;

impl LiveBackendStartupReport {
    pub fn native_libdrm_output_routes(&self) -> Vec<LibdrmNativeOutputRoute> {
        self.discovery
            .outputs
            .outputs()
            .enumerate()
            .filter_map(|(index, output)| {
                LibdrmNativeOutputSlot::new(
                    u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX),
                )
                .map(|slot| LibdrmNativeOutputRoute {
                    slot,
                    output: output.output,
                })
            })
            .collect()
    }

    pub fn native_libdrm_poller_startup_report(&self) -> LiveLibdrmPollerStartupReport {
        if !self.discovery.is_ready() {
            return LiveLibdrmPollerStartupReport {
                status: if self.discovery.selected_output.is_none() {
                    LiveLibdrmPollerStartupStatus::NoOutputs
                } else {
                    LiveLibdrmPollerStartupStatus::BackendNotReady
                },
                route_count: 0,
            };
        }

        let route_count = self.native_libdrm_output_routes().len();
        LiveLibdrmPollerStartupReport {
            status: if route_count == 0 {
                LiveLibdrmPollerStartupStatus::NoOutputs
            } else {
                LiveLibdrmPollerStartupStatus::Ready
            },
            route_count,
        }
    }

    pub fn native_libdrm_poller_from_authority(
        &self,
        authority: LibdrmBackendFdAuthority,
    ) -> Option<NativeLibdrmPageFlipEventPoller> {
        if !self.discovery.is_ready() {
            return None;
        }

        Some(
            NativeLibdrmPageFlipEventPoller::new(LibdrmNativePageFlipSource::from_authority(
                authority,
            ))
            .with_routes(self.native_libdrm_output_routes()),
        )
    }
}
