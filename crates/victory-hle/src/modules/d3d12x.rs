//! `d3d12_xs.dll`: virtual memory setup, device creation, root signature
//! serialization. The device and everything it creates live in
//! `victory-gpu`.

use std::ffi::c_void;

use victory_gpu::device;
use victory_gpu::object::{HRESULT, Iid, Object};

use crate::{HleExport, HleModule};

/// Device creation; first hands the device the title's general allocator
/// (`XMemAlloc`, which honours the title's hooks) for resource memory.
extern "system" fn create_device(
    adapter: *const c_void,
    parameters: *const u32,
    iid: *const Iid,
    out: *mut *const Object,
) -> HRESULT {
    victory_gpu::resource::use_allocator(super::xmem::XMemAlloc, super::xmem::XMemFree);
    victory_gpu::resource::use_page_mapper(super::xmem::map_alias);
    #[cfg(windows)]
    victory_gpu::on_failure(host_gpu_failed);
    device::create(adapter, parameters, iid, out)
}

/// A host GPU failure ends the run like a crash: reported to the crash log,
/// then the process ends as the console ends a title.
#[cfg(windows)]
fn host_gpu_failed(what: &str) -> ! {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
    victory_loader::log_crash(&format!("VIctory: host GPU failure: {what}"));
    const DXGI_ERROR_DEVICE_REMOVED: u32 = 0x887A_0005;
    // SAFETY: ends our own process.
    unsafe { TerminateProcess(GetCurrentProcess(), DXGI_ERROR_DEVICE_REMOVED) };
    unreachable!("the process has ended")
}

/// The offline warm-up: see `victory_gpu::shader_cache::warm`.
pub fn warm_shader(container: &[u8]) -> Result<bool, String> {
    #[cfg(windows)]
    {
        victory_gpu::shader_cache::warm(container)
    }
    #[cfg(not(windows))]
    {
        let _ = container;
        Err("no shader cache on this host".into())
    }
}

/// Keeps the host's translations of the title's shaders in `dir` (a
/// folder of the cache) across runs, and its compiled pipelines beside it
/// (`pipelines`).
pub fn use_shader_cache(dir: std::path::PathBuf) {
    #[cfg(windows)]
    {
        victory_gpu::pipeline::use_pipeline_cache(dir.with_file_name("pipelines"));
        victory_gpu::shader_cache::use_shader_cache(dir);
    }
    #[cfg(not(windows))]
    let _ = dir;
}

/// `victory prepare`: the container's translation put in the shader cache
/// [`use_shader_cache`] opened, as a run on this PC makes it. Why it does
/// not translate, if not.
#[cfg(windows)]
pub fn prepare_shader(container: &[u8]) -> Result<(), String> {
    victory_gpu::shader_cache::prepare(container)
}

/// Off Windows nothing runs: no translation is kept.
#[cfg(not(windows))]
pub fn prepare_shader(_container: &[u8]) -> Result<(), String> {
    Err("no host on this system".into())
}

/// `victory prepare`: makes the host pipeline of every recipe earlier runs
/// kept (each pipeline a draw or dispatch asked for) into the pipeline
/// library [`use_shader_cache`] opened, on `threads` workers; `say` gets a
/// progress line every 15 s and a last one. Whether there were recipes.
#[cfg(windows)]
pub fn prepare_pipelines(threads: usize, say: &(dyn Fn(String) + Sync)) -> bool {
    let line = |p: &victory_gpu::pipeline::Prepared, last: bool| {
        say(format!(
            "{} {} of {} pipelines ready ({} already in the library, {} compiled now), {} cannot be made",
            if last { "prepared    " } else { "preparing   " },
            p.loaded + p.made,
            p.usable,
            p.loaded,
            p.made,
            p.failed
        ));
        // Try 6.23: the recipes not made, by why (none of them is a pipeline
        // a run would make: those are all above).
        if last && p.recipes > p.usable {
            say(format!(
                "prepared     {} recipes not made: {} with a shader no run kept, {} of programs a run leaves out (rays only, denied, skipped), {} with a graphics state that does not read",
                p.recipes - p.usable,
                p.no_container,
                p.left_out,
                p.bad_state
            ));
        }
    };
    victory_gpu::pipeline::prepare_recipes(threads, &line).recipes > 0
}

/// Off Windows nothing runs: there are no recipes.
#[cfg(not(windows))]
pub fn prepare_pipelines(_threads: usize, _say: &(dyn Fn(String) + Sync)) -> bool {
    false
}

/// Translations found in the shader cache and made, since the start.
#[cfg(windows)]
pub fn prepared_shader_counts() -> (u64, u64) {
    victory_gpu::shader_cache::prepared_counts()
}

/// Off Windows nothing runs.
#[cfg(not(windows))]
pub fn prepared_shader_counts() -> (u64, u64) {
    (0, 0)
}

pub(crate) fn module() -> HleModule {
    HleModule {
        name: "d3d12x",
        dlls: &["d3d12_xs.dll"],
        exports: vec![
            HleExport {
                name: "D3D12ConfigureVirtualMemoryX",
                address: device::configure_virtual_memory as *const () as usize,
            },
            HleExport {
                name: "D3D12XboxCreateDevice",
                address: create_device as *const () as usize,
            },
            HleExport {
                name: "D3D12SerializeRootSignature",
                address: victory_gpu::root::serialize as *const () as usize,
            },
            HleExport {
                name: "D3D12SerializeVersionedRootSignature",
                address: victory_gpu::root::serialize_versioned as *const () as usize,
            },
            // The AMD upscaler's (Series S).
            HleExport {
                name: "D3D12CreateRootSignatureDeserializer",
                address: victory_gpu::root::create_deserializer as *const () as usize,
            },
            // A data import: the title reads the id at this address.
            HleExport {
                name: "IID_ID3D12Device",
                address: (&raw const device::IID) as usize,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HleResolver, HostPolicy};
    use victory_loader::ImportResolver;
    use victory_loader::Resolution;
    use victory_pe::ImportSymbol;

    #[test]
    fn the_interface_id_import_is_bound_to_data() {
        let symbol = ImportSymbol::Name {
            hint: 0,
            name: "IID_ID3D12Device".into(),
        };
        let r = HleResolver::new(HostPolicy::none());
        let Resolution::Found { address, .. } = r.resolve("d3d12_xs.dll", &symbol) else {
            panic!("not bound");
        };
        // SAFETY: the address is our static id.
        assert_eq!(unsafe { *(address as *const [u8; 16]) }, device::IID);
    }
}
