//! Host work that a host library records into a title's command list in
//! place of a console one (the host's FidelityFX upscaler, say): the host
//! device, a title texture's host copy in the state the library is told,
//! and the host list itself.
//!
//! From Belobug/VIctory cd8dfc06 (ported there from DexNom/VIctory
//! ab34a8b2); here on the plain host list of each title list (Try 6.10).

use windows::Win32::Graphics::Direct3D12::{ID3D12Device, ID3D12GraphicsCommandList, ID3D12Resource};

use crate::object::Object;

/// The host D3D12 device, created on first use; `None` without a host GPU.
pub fn host_device() -> Option<ID3D12Device> {
    crate::host::host().map(|h| h.device.clone())
}

/// The active host list for one synchronous external recording call.
pub struct Recording<'a>(&'a crate::host::HostList);

impl Recording<'_> {
    /// Move a title texture to the state the provider will see. The list
    /// holds the backing resource through submission.
    pub fn texture(&self, resource: *const Object, state: u32) -> Option<(ID3D12Resource, u32)> {
        let texture = crate::resource::host_resource(resource)?.clone();
        crate::list::ensure_state(self.0, resource, state);
        let state = crate::host::legal_state(&texture, state);
        Some((texture, state))
    }

    /// Prepare aliases, uploads and depth twins as for a compute read.
    pub fn prepare_input(&self, resource: *const Object) -> Option<()> {
        let texture = crate::resource::host_resource(resource)?.clone();
        let aliased = crate::resource::alias_of(resource)
            .is_some_and(|source| crate::texture::alias(self.0, resource, source.get()));
        if !aliased && crate::texture::title_written(&texture) {
            crate::texture::upload(self.0, resource, &texture);
        }
        let _ = self.0.twin_use(resource, false, false);
        Some(())
    }

    /// The host command list the provider records into.
    pub fn commands(&self) -> &ID3D12GraphicsCommandList {
        &self.0.list
    }
}

/// The producer recorded its output on the host's command list.
pub fn output_written(resource: *const Object) {
    crate::texture::host_written(resource);
}

/// Settle submitted title work before releasing a provider's private GPU
/// resources.
pub fn finish_submitted() {
    crate::queue::before_sync();
    crate::host::settle_for_fence();
}

/// Runs `f` with the host list behind the title list `list` (open for
/// recording); `None` without one. What `f` binds (descriptor heaps, root
/// signatures, pipeline) is replaced before the list's next draw or
/// dispatch: the draw state is replayed afterwards, as after our own
/// upscale passes (`list::upscale_temporal`).
pub fn record<R>(list: *const Object, f: impl FnOnce(&Recording<'_>) -> R) -> Option<R> {
    if list.is_null() {
        return None;
    }
    let mut out = None;
    crate::list::with_state(list, |host, state| {
        out = Some(f(&Recording(host)));
        state.replay(host);
    });
    out
}

/// A title texture's host copy, moved to the host resource `state` in
/// `list`, and the state it is in: `state` without the bits the host copy
/// cannot take (see `host::legal_state`). For `crate::streamline`, which
/// records on the host list directly (`crate::list::external`).
pub(crate) fn host_texture(list: &crate::host::HostList, resource: *const Object, state: u32) -> Option<(ID3D12Resource, u32)> {
    Recording(list).texture(resource, state)
}

/// Prepares a title-authored input with the alias, upload and depth twin
/// rules a compute shader's resource view has (as [`Recording::prepare_input`]).
pub(crate) fn prepare_input(list: &crate::host::HostList, resource: *const Object) -> Option<()> {
    Recording(list).prepare_input(resource)
}
