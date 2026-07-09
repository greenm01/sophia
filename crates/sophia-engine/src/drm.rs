use crate::HeadlessOutput;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrmKmsMode {
    pub size: Size,
    pub refresh_millihz: u32,
}

impl DrmKmsMode {
    pub const fn new(width: i32, height: i32, refresh_millihz: u32) -> Self {
        Self {
            size: Size { width, height },
            refresh_millihz,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrmKmsOutputDescriptor {
    pub output: OutputId,
    pub connector_id: u32,
    pub crtc_id: u32,
    pub mode: DrmKmsMode,
    pub scale: u32,
}

impl DrmKmsOutputDescriptor {
    pub const fn as_engine_output(self) -> HeadlessOutput {
        HeadlessOutput {
            id: self.output,
            size: self.mode.size,
            scale: self.scale,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DrmKmsOutputRegistry {
    outputs: BTreeMap<OutputId, DrmKmsOutputDescriptor>,
}

impl DrmKmsOutputRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&mut self, output: DrmKmsOutputDescriptor) {
        self.outputs.insert(output.output, output);
    }

    pub fn remove(&mut self, output: OutputId) -> Option<DrmKmsOutputDescriptor> {
        self.outputs.remove(&output)
    }

    pub fn get(&self, output: OutputId) -> Option<&DrmKmsOutputDescriptor> {
        self.outputs.get(&output)
    }

    pub fn outputs(&self) -> impl Iterator<Item = &DrmKmsOutputDescriptor> {
        self.outputs.values()
    }

    pub fn primary_engine_output(&self) -> Option<HeadlessOutput> {
        self.outputs
            .values()
            .next()
            .map(|output| output.as_engine_output())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrmKmsDiscoveryConfig {
    pub default_refresh_millihz: u32,
    pub default_scale: u32,
}

impl Default for DrmKmsDiscoveryConfig {
    fn default() -> Self {
        Self {
            default_refresh_millihz: 60_000,
            default_scale: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DrmKmsSysfsDiscovery {
    config: DrmKmsDiscoveryConfig,
}

impl DrmKmsSysfsDiscovery {
    pub fn new(config: DrmKmsDiscoveryConfig) -> Self {
        Self { config }
    }

    pub fn discover_outputs(&self, root: impl AsRef<Path>) -> io::Result<DrmKmsOutputRegistry> {
        let mut registry = DrmKmsOutputRegistry::new();
        for (index, path) in drm_connector_paths(root.as_ref())?.into_iter().enumerate() {
            let Some(descriptor) = self.discover_connector(&path, index)? else {
                continue;
            };
            registry.upsert(descriptor);
        }
        Ok(registry)
    }

    fn discover_connector(
        &self,
        path: &Path,
        index: usize,
    ) -> io::Result<Option<DrmKmsOutputDescriptor>> {
        if read_trimmed(path.join("status"))?.as_deref() != Some("connected") {
            return Ok(None);
        }

        let Some(mode) = read_first_mode(path, self.config.default_refresh_millihz)? else {
            return Ok(None);
        };
        let output = OutputId::from_raw(u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX));
        let connector_id = read_u32_file(path.join("connector_id"))?
            .unwrap_or_else(|| u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX));
        let crtc_id = read_u32_file(path.join("crtc_id"))?.unwrap_or(0);
        let scale = read_u32_file(path.join("scale"))?.unwrap_or(self.config.default_scale);

        Ok(Some(DrmKmsOutputDescriptor {
            output,
            connector_id,
            crtc_id,
            mode,
            scale: scale.max(1),
        }))
    }
}

pub fn discover_drm_kms_outputs_from_sysfs(
    root: impl AsRef<Path>,
) -> io::Result<DrmKmsOutputRegistry> {
    DrmKmsSysfsDiscovery::default().discover_outputs(root)
}

fn drm_connector_paths(root: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("card") && name.contains('-') {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn read_first_mode(path: &Path, default_refresh_millihz: u32) -> io::Result<Option<DrmKmsMode>> {
    let Some(contents) = read_trimmed(path.join("modes"))? else {
        return Ok(None);
    };
    let Some(line) = contents.lines().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    Ok(parse_drm_mode(line.trim(), default_refresh_millihz))
}

fn parse_drm_mode(mode: &str, refresh_millihz: u32) -> Option<DrmKmsMode> {
    let (width, height) = mode.split_once('x')?;
    Some(DrmKmsMode::new(
        width.parse().ok()?,
        height.parse().ok()?,
        refresh_millihz,
    ))
    .filter(|mode| mode.size.width > 0 && mode.size.height > 0)
}

fn read_u32_file(path: impl AsRef<Path>) -> io::Result<Option<u32>> {
    Ok(read_trimmed(path)?
        .as_deref()
        .and_then(|contents| contents.parse().ok()))
}

fn read_trimmed(path: impl AsRef<Path>) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents.trim().to_owned())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
