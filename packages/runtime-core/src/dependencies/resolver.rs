use super::*;
use node_semver::{Range, Version};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const MAX_CATALOG_ENTRIES: usize = 512;
const MAX_PACKAGES: usize = 128;
const MAX_EDGES: usize = 4096;
const MAX_SEARCH_STEPS: usize = 10_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ServiceDefinition {
    id: String,
    version: String,
    operations: Vec<Value>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ServiceDependency {
    publisher: String,
    id: String,
    version: String,
    service: Option<String>,
    interface_version: Option<String>,
}
impl ServiceDependency {
    fn identity(&self) -> DependencyIdentity {
        DependencyIdentity {
            publisher: self.publisher.clone(),
            id: self.id.clone(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToolDependency {
    publisher: String,
    id: String,
    version: String,
    target: Option<DependencyTarget>,
    build_flavor: Option<String>,
    digest: Option<String>,
}
/// Internal fixed-catalog descriptor. No paths, loader functions or executables.
#[derive(Clone, Debug)]
pub struct PackageCandidate {
    pin: PackagePin,
    targets: Vec<DependencyTarget>,
    services: Vec<ServiceDefinition>,
    service_dependencies: Vec<ServiceDependency>,
    tool_dependencies: Vec<ToolDependency>,
}
/// Build-owned/disposable fixture tool metadata, never a tool execution request.
#[derive(Clone, Debug)]
pub struct ToolCandidate {
    pub publisher: String,
    pub id: String,
    pub version: String,
    pub target: DependencyTarget,
    pub build_flavor: String,
    pub digest: String,
}
fn id(value: &str) -> bool {
    valid_dependency_id(value)
}
pub(crate) fn valid_dependency_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn target_is_valid(value: &DependencyTarget) -> bool {
    matches!(
        value.platform.as_str(),
        "windows" | "macos" | "linux" | "web"
    ) && matches!(value.arch.as_str(), "x64" | "arm64" | "wasm32")
}
fn numeric_prereleases_supported(value: &str) -> bool {
    value
        .split("||")
        .flat_map(str::split_whitespace)
        .all(|part| {
            // Build metadata is ignored by npm range comparison. A prerelease
            // identifier is numeric only when the complete dot-separated token is
            // decimal; alpha900... and alpha-900... remain ordinary identifiers.
            let before_build = part.split('+').next().unwrap_or(part);
            let Some((_, prerelease)) = before_build.split_once('-') else {
                return true;
            };
            prerelease.split('.').all(|identifier| {
                !identifier.bytes().all(|b| b.is_ascii_digit())
                    || identifier.is_empty()
                    || (!(identifier.len() > 1 && identifier.starts_with('0'))
                        && identifier
                            .parse::<u64>()
                            .is_ok_and(|number| number <= 900_719_925_474_099))
            })
        })
}
fn range_source_supported(value: &str) -> bool {
    // node-semver's parser admits several loose spellings. SDK uses npm's
    // strict syntax authority; reject leading core zeros and a prerelease
    // lacking '-' before the library performs range expansion.
    value
        .split("||")
        .flat_map(str::split_whitespace)
        .all(|part| {
            if part == "-" {
                return true;
            }
            let part = part.trim_start_matches(['<', '>', '=', '~', '^']);
            if part.is_empty() {
                return true;
            }
            let part = part.strip_prefix('v').unwrap_or(part);
            let core = part.split(['-', '+']).next().unwrap_or(part);
            core.split('.').all(|component| {
                matches!(component, "*" | "x" | "X")
                    || (!component.is_empty()
                        && component.bytes().all(|b| b.is_ascii_digit())
                        && !(component.len() > 1 && component.starts_with('0'))
                        && component
                            .parse::<u64>()
                            .is_ok_and(|number| number <= 900_719_925_474_099))
            })
        })
}
fn version(value: &str) -> Result<Version, DependencyError> {
    // SDK canonical exact-version declarations use semver.valid(v) === v,
    // whose normalized value excludes build metadata. Ranges/matching still
    // use npm semantics, including ignoring build metadata in comparisons.
    if value.len() > 128 || value.contains('+') || !numeric_prereleases_supported(value) {
        return Err(DependencyError::InvalidDeclaration);
    }
    let parsed: Version = value
        .parse()
        .map_err(|_| DependencyError::InvalidDeclaration)?;
    if parsed.to_string() != value {
        return Err(DependencyError::InvalidDeclaration);
    }
    Ok(parsed)
}
pub(super) struct NpmRange(Vec<Range>);
impl NpmRange {
    pub(super) fn satisfies(&self, version: &Version) -> bool {
        self.0.iter().any(|branch| branch.satisfies(version))
    }
}
fn npm_atom(value: &str) -> Result<Option<Range>, DependencyError> {
    let operator = if value.starts_with("<=") {
        "<="
    } else if value.starts_with(">=") {
        ">="
    } else if value.starts_with('<') {
        "<"
    } else if value.starts_with('>') {
        ">"
    } else {
        ""
    };
    if operator.is_empty() {
        return value
            .parse()
            .map(Some)
            .map_err(|_| DependencyError::InvalidDeclaration);
    }
    let operand = value[operator.len()..]
        .strip_prefix('v')
        .unwrap_or(&value[operator.len()..]);
    let core = operand.split(['-', '+']).next().unwrap_or(operand);
    let parts: Vec<_> = core.split('.').collect();
    let count = parts
        .iter()
        .position(|part| matches!(*part, "x" | "X" | "*"))
        .unwrap_or(parts.len());
    if count >= 3 {
        return value
            .parse()
            .map(Some)
            .map_err(|_| DependencyError::InvalidDeclaration);
    }
    if count == 0 {
        return Ok(if matches!(operator, ">" | "<") {
            None
        } else {
            Some(Range::any())
        });
    }
    let mut core = [0u64; 3];
    for n in 0..count {
        core[n] = parts[n]
            .parse()
            .map_err(|_| DependencyError::InvalidDeclaration)?;
    }
    let normalized = match operator {
        ">=" => format!(">={}.{}.{}", core[0], core[1], core[2]),
        "<" => format!("<{}.{}.{}-0", core[0], core[1], core[2]),
        ">" | "<=" => {
            core[count - 1] += 1;
            if core[count - 1] > 900_719_925_474_099 {
                // Only a synthetic bound crosses the supported source domain.
                // The crate constructs it from a >partial comparison without
                // parsing an out-of-domain caller-supplied core component.
                let greater: Range = format!(">{operand}")
                    .parse()
                    .map_err(|_| DependencyError::InvalidDeclaration)?;
                return Ok(if operator == ">" {
                    Some(greater)
                } else {
                    Range::any().difference(&greater)
                });
            }
            format!(
                "{}{}.{}.{}{}",
                if operator == ">" { ">=" } else { "<" },
                core[0],
                core[1],
                core[2],
                if operator == "<=" { "-0" } else { "" }
            )
        }
        _ => unreachable!(),
    };
    normalized
        .parse()
        .map(Some)
        .map_err(|_| DependencyError::InvalidDeclaration)
}
pub(super) fn range(value: &str) -> Result<NpmRange, DependencyError> {
    if value.is_empty()
        || value.len() > 256
        || !numeric_prereleases_supported(value)
        || !range_source_supported(value)
    {
        return Err(DependencyError::InvalidDeclaration);
    }
    let mut branches = Vec::new();
    for branch in value.split("||") {
        let tokens: Vec<_> = branch.split_whitespace().collect();
        let mut intersection = Some(Range::any());
        let mut n = 0;
        while n < tokens.len() {
            let (atom, consumed) = if n + 2 < tokens.len() && tokens[n + 1] == "-" {
                (format!("{} - {}", tokens[n], tokens[n + 2]), 3)
            } else if matches!(tokens[n], "<" | ">" | "<=" | ">=" | "=" | "~" | "^" | "~>") {
                let operand = tokens
                    .get(n + 1)
                    .ok_or(DependencyError::InvalidDeclaration)?;
                (format!("{}{operand}", tokens[n]), 2)
            } else {
                (tokens[n].to_owned(), 1)
            };
            let next = npm_atom(&atom)?;
            intersection =
                intersection.and_then(|existing| next.and_then(|next| existing.intersect(&next)));
            n += consumed;
        }
        if let Some(branch) = intersection {
            branches.push(branch);
        }
    }
    // Explicit intersection avoids the crate's loose fallback which turns
    // contradictory comparators into a union. Empty branches match no version.
    Ok(NpmRange(branches))
}
fn string(value: &Value, key: &str) -> Result<String, DependencyError> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or(DependencyError::InvalidDeclaration)
}

impl PackageCandidate {
    /// The complete manifest remains bound by its digest. Semantic declaration
    /// parsing is defensive; package admission still belongs to the Host build.
    pub fn from_manifest(manifest: &Value) -> Result<Self, DependencyError> {
        if serde_json::to_vec(manifest)
            .map_err(|_| DependencyError::InvalidDeclaration)?
            .len()
            > crate::protocol::MAX_FRAME_BYTES
        {
            return Err(DependencyError::BudgetExceeded);
        }
        let pin = PackagePin {
            publisher: string(manifest, "publisher")?,
            id: string(manifest, "id")?,
            version: string(manifest, "version")?,
            digest: crate::catalog::digest(manifest),
        };
        if !id(&pin.publisher) || !id(&pin.id) {
            return Err(DependencyError::InvalidDeclaration);
        }
        version(&pin.version)?;
        let targets: Vec<DependencyTarget> = serde_json::from_value(manifest["targets"].clone())
            .map_err(|_| DependencyError::InvalidDeclaration)?;
        if targets.is_empty()
            || targets.len() > 16
            || targets.iter().any(|t| !target_is_valid(t))
            || targets.iter().collect::<BTreeSet<_>>().len() != targets.len()
        {
            return Err(DependencyError::InvalidDeclaration);
        }
        let services: Vec<ServiceDefinition> = match manifest.get("services") {
            None => Vec::new(),
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|_| DependencyError::InvalidDeclaration)?,
        };
        if services.len() > 64 {
            return Err(DependencyError::BudgetExceeded);
        }
        if !services.is_empty() {
            let entry = manifest["entries"]["services"]
                .as_str()
                .ok_or(DependencyError::InvalidDeclaration)?;
            if !manifest["files"].as_array().is_some_and(|files| {
                files
                    .iter()
                    .any(|file| file["path"].as_str() == Some(entry))
            }) {
                return Err(DependencyError::InvalidDeclaration);
            }
        }
        let mut provided = BTreeSet::new();
        for service in &services {
            if !id(&service.id)
                || !provided.insert(&service.id)
                || service.operations.is_empty()
                || service.operations.len() > 64
            {
                return Err(DependencyError::InvalidDeclaration);
            }
            version(&service.version)?;
            let mut operations = BTreeSet::new();
            for operation in &service.operations {
                let operation_id = operation["id"]
                    .as_str()
                    .ok_or(DependencyError::InvalidDeclaration)?;
                if !id(operation_id)
                    || !operations.insert(operation_id)
                    || operation["headless"] != true
                    || operation["interaction"] != "none"
                {
                    return Err(DependencyError::InvalidDeclaration);
                }
            }
        }
        let service_dependencies: Vec<ServiceDependency> =
            serde_json::from_value(manifest["dependencies"]["services"].clone())
                .map_err(|_| DependencyError::InvalidDeclaration)?;
        let tool_dependencies: Vec<ToolDependency> =
            serde_json::from_value(manifest["dependencies"]["tools"].clone())
                .map_err(|_| DependencyError::InvalidDeclaration)?;
        if service_dependencies.len() > 128 || tool_dependencies.len() > 128 {
            return Err(DependencyError::BudgetExceeded);
        }
        let mut consumed = BTreeSet::new();
        let mut legacy = BTreeSet::new();
        let mut selected = BTreeSet::new();
        for dep in &service_dependencies {
            if !id(&dep.publisher) || !id(&dep.id) {
                return Err(DependencyError::InvalidDeclaration);
            }
            range(&dep.version)?;
            match (&dep.service, &dep.interface_version) {
                (Some(service), Some(interface)) if id(service) => {
                    range(interface)?;
                    selected.insert(dep.identity());
                    if !consumed.insert((dep.identity(), Some(service.clone()))) {
                        return Err(DependencyError::InvalidDeclaration);
                    }
                }
                (None, None) => {
                    legacy.insert(dep.identity());
                    if !consumed.insert((dep.identity(), None)) {
                        return Err(DependencyError::InvalidDeclaration);
                    }
                }
                _ => return Err(DependencyError::InvalidDeclaration),
            }
        }
        if !legacy.is_disjoint(&selected) {
            return Err(DependencyError::InvalidDeclaration);
        }
        let mut tools = BTreeSet::new();
        let mut legacy_tools = BTreeSet::new();
        let mut selected_tools = BTreeSet::new();
        for dep in &tool_dependencies {
            if !id(&dep.publisher) || !id(&dep.id) {
                return Err(DependencyError::InvalidDeclaration);
            }
            range(&dep.version)?;
            match (&dep.target, &dep.build_flavor, &dep.digest) {
                (Some(t), Some(flavor), Some(digest))
                    if target_is_valid(t) && id(flavor) && hash(digest) =>
                {
                    selected_tools.insert((&dep.publisher, &dep.id));
                }
                (None, None, None) => {
                    legacy_tools.insert((&dep.publisher, &dep.id));
                }
                _ => return Err(DependencyError::InvalidDeclaration),
            }
            if !tools.insert((&dep.publisher, &dep.id, &dep.target, &dep.build_flavor)) {
                return Err(DependencyError::InvalidDeclaration);
            }
        }
        if !legacy_tools.is_disjoint(&selected_tools) {
            return Err(DependencyError::InvalidDeclaration);
        }
        Ok(Self {
            pin,
            targets,
            services,
            service_dependencies,
            tool_dependencies,
        })
    }
}

pub struct DependencyCatalog {
    packages: BTreeMap<DependencyIdentity, Vec<PackageCandidate>>,
    tools: Vec<ToolCandidate>,
}
impl DependencyCatalog {
    pub fn from_manifests(
        manifests: Vec<Value>,
        tools: Vec<ToolCandidate>,
    ) -> Result<Self, DependencyError> {
        if manifests.len() + tools.len() > MAX_CATALOG_ENTRIES {
            return Err(DependencyError::BudgetExceeded);
        }
        Self::new(
            manifests
                .iter()
                .map(PackageCandidate::from_manifest)
                .collect::<Result<Vec<_>, _>>()?,
            tools,
        )
    }
    pub fn new(
        packages: Vec<PackageCandidate>,
        mut tools: Vec<ToolCandidate>,
    ) -> Result<Self, DependencyError> {
        if packages.len() + tools.len() > MAX_CATALOG_ENTRIES {
            return Err(DependencyError::BudgetExceeded);
        }
        let mut inventory: BTreeMap<DependencyIdentity, Vec<PackageCandidate>> = BTreeMap::new();
        for package in packages {
            let group = inventory.entry(package.pin.identity()).or_default();
            if let Some(existing) = group
                .iter()
                .find(|item| item.pin.version == package.pin.version)
            {
                return Err(if existing.pin.digest == package.pin.digest {
                    DependencyError::DuplicateProvider
                } else {
                    DependencyError::ArtifactConflict
                });
            }
            group.push(package);
        }
        for group in inventory.values_mut() {
            group.sort_by(|a, b| {
                version(&b.pin.version)
                    .unwrap()
                    .cmp(&version(&a.pin.version).unwrap())
                    .then_with(|| a.pin.version.cmp(&b.pin.version))
            });
        }
        let mut tool_ids = BTreeMap::new();
        for tool in &tools {
            if !id(&tool.publisher)
                || !id(&tool.id)
                || !target_is_valid(&tool.target)
                || !id(&tool.build_flavor)
                || !hash(&tool.digest)
            {
                return Err(DependencyError::InvalidDeclaration);
            }
            version(&tool.version)?;
            let key = (
                &tool.publisher,
                &tool.id,
                &tool.version,
                &tool.target,
                &tool.build_flavor,
            );
            if let Some(existing) = tool_ids.insert(key, &tool.digest) {
                return Err(if existing == &tool.digest {
                    DependencyError::DuplicateProvider
                } else {
                    DependencyError::ArtifactConflict
                });
            }
        }
        tools.sort_by(|a, b| {
            (&a.publisher, &a.id)
                .cmp(&(&b.publisher, &b.id))
                .then_with(|| {
                    version(&b.version)
                        .unwrap()
                        .cmp(&version(&a.version).unwrap())
                })
                .then_with(|| {
                    (&a.version, &a.target, &a.build_flavor, &a.digest).cmp(&(
                        &b.version,
                        &b.target,
                        &b.build_flavor,
                        &b.digest,
                    ))
                })
        });
        Ok(Self {
            packages: inventory,
            tools,
        })
    }
    pub fn resolve(
        &self,
        roots: &[DependencyIdentity],
        target: &DependencyTarget,
    ) -> Result<DependencyPlan, DependencyError> {
        if roots.is_empty() {
            return Err(DependencyError::InvalidDeclaration);
        }
        if roots.len() > MAX_PACKAGES {
            return Err(DependencyError::BudgetExceeded);
        }
        if !target_is_valid(target)
            || roots
                .iter()
                .any(|root| !id(&root.publisher) || !id(&root.id))
        {
            return Err(DependencyError::InvalidDeclaration);
        }
        let unique: BTreeSet<_> = roots.iter().cloned().collect();
        if unique.len() != roots.len() {
            return Err(DependencyError::InvalidDeclaration);
        }
        let roots: Vec<_> = unique.into_iter().collect();
        let mut budget = MAX_SEARCH_STEPS;
        let selected = self.search(&roots, target, BTreeMap::new(), &mut budget)?;
        self.plan(&roots, target, &selected)
    }
    fn search<'a>(
        &'a self,
        roots: &[DependencyIdentity],
        target: &DependencyTarget,
        selected: BTreeMap<DependencyIdentity, &'a PackageCandidate>,
        budget: &mut usize,
    ) -> Result<BTreeMap<DependencyIdentity, &'a PackageCandidate>, DependencyError> {
        if *budget == 0 {
            return Err(DependencyError::BudgetExceeded);
        }
        *budget -= 1;
        if selected.len() > MAX_PACKAGES {
            return Err(DependencyError::BudgetExceeded);
        }
        let mut needed: BTreeSet<DependencyIdentity> = roots.iter().cloned().collect();
        let mut constraints: BTreeMap<DependencyIdentity, Vec<&ServiceDependency>> =
            BTreeMap::new();
        let mut edges = 0;
        for package in selected.values() {
            for dep in &package.service_dependencies {
                if dep.service.is_none() {
                    return Err(DependencyError::InterfaceRequired);
                }
                needed.insert(dep.identity());
                constraints.entry(dep.identity()).or_default().push(dep);
                edges += 1;
            }
        }
        if needed.len() > MAX_PACKAGES || edges > MAX_EDGES {
            return Err(DependencyError::BudgetExceeded);
        }
        for (identity, package) in &selected {
            if !satisfies(
                package,
                constraints.get(identity).map(Vec::as_slice).unwrap_or(&[]),
            )? {
                return Err(DependencyError::VersionConflict);
            }
        }
        if service_cycle(&selected) {
            return Err(DependencyError::Cycle);
        }
        let Some(next) = needed
            .iter()
            .find(|identity| !selected.contains_key(*identity))
        else {
            // A complete graph must also fit the response budget before it is
            // accepted. Larger metadata on a higher version may backtrack to a
            // smaller valid lock, just like structural graph bounds.
            self.plan(roots, target, &selected)?;
            return Ok(selected);
        };
        let group = self.packages.get(next).ok_or(DependencyError::Missing)?;
        let compatible: Vec<_> = group
            .iter()
            .filter(|package| package.targets.contains(target))
            .collect();
        if compatible.is_empty() {
            return Err(DependencyError::TargetMismatch);
        }
        let constraints = constraints.get(next).map(Vec::as_slice).unwrap_or(&[]);
        let mut last_error = DependencyError::VersionConflict;
        for package in compatible {
            if !satisfies(package, constraints)? {
                continue;
            }
            let mut branch = selected.clone();
            branch.insert(next.clone(), package);
            match self.search(roots, target, branch, budget) {
                Ok(solution) => return Ok(solution),
                // Exhausted global search fuel stops all alternatives. A
                // candidate whose graph exceeds structural bounds can still
                // backtrack to a smaller valid version within the same fuel.
                Err(DependencyError::BudgetExceeded) if *budget == 0 => {
                    return Err(DependencyError::BudgetExceeded)
                }
                Err(error) => last_error = error,
            }
        }
        Err(last_error)
    }
    fn tool_pins(
        &self,
        host_target: &DependencyTarget,
        selected: &BTreeMap<DependencyIdentity, &PackageCandidate>,
    ) -> Result<Vec<ToolPin>, DependencyError> {
        let mut result = Vec::new();
        for (consumer, package) in selected {
            let mut target_groups = BTreeMap::<_, bool>::new();
            for dep in &package.tool_dependencies {
                let Some(flavor) = dep.build_flavor.as_ref() else {
                    return Err(DependencyError::InvalidDeclaration);
                };
                let group = target_groups
                    .entry((&dep.publisher, &dep.id, flavor))
                    .or_default();
                *group |= dep.target.as_ref() == Some(host_target);
            }
            if target_groups.values().any(|matched| !matched) {
                return Err(DependencyError::TargetMismatch);
            }
            for dep in &package.tool_dependencies {
                let (Some(target), Some(flavor), Some(digest)) =
                    (&dep.target, &dep.build_flavor, &dep.digest)
                else {
                    return Err(DependencyError::InvalidDeclaration);
                };
                if target != host_target {
                    continue;
                }
                let candidates: Vec<_> = self
                    .tools
                    .iter()
                    .filter(|tool| tool.publisher == dep.publisher && tool.id == dep.id)
                    .collect();
                if candidates.is_empty() {
                    return Err(DependencyError::Missing);
                }
                let candidates: Vec<_> = candidates
                    .into_iter()
                    .filter(|tool| tool.target == *target)
                    .collect();
                if candidates.is_empty() {
                    return Err(DependencyError::TargetMismatch);
                }
                let required = range(&dep.version)?;
                let candidates: Vec<_> = candidates
                    .into_iter()
                    .filter(|tool| required.satisfies(&version(&tool.version).unwrap()))
                    .collect();
                if candidates.is_empty() {
                    return Err(DependencyError::VersionConflict);
                }
                let candidate = candidates
                    .into_iter()
                    .find(|tool| tool.build_flavor == *flavor && tool.digest == *digest)
                    .ok_or(DependencyError::ArtifactConflict)?;
                result.push(ToolPin {
                    consumer: consumer.clone(),
                    publisher: candidate.publisher.clone(),
                    id: candidate.id.clone(),
                    version: candidate.version.clone(),
                    target: candidate.target.clone(),
                    build_flavor: candidate.build_flavor.clone(),
                    digest: candidate.digest.clone(),
                });
            }
        }
        result.sort();
        if result.len() > MAX_EDGES {
            return Err(DependencyError::BudgetExceeded);
        }
        Ok(result)
    }
    fn plan(
        &self,
        roots: &[DependencyIdentity],
        target: &DependencyTarget,
        selected: &BTreeMap<DependencyIdentity, &PackageCandidate>,
    ) -> Result<DependencyPlan, DependencyError> {
        let mut services = Vec::new();
        let mut edges = Vec::new();
        let mut reverse: BTreeMap<_, BTreeSet<_>> = selected
            .keys()
            .cloned()
            .map(|key| (key, BTreeSet::new()))
            .collect();
        for (consumer, package) in selected {
            for dep in &package.service_dependencies {
                let provider = selected
                    .get(&dep.identity())
                    .ok_or(DependencyError::Missing)?;
                let service = provider
                    .services
                    .iter()
                    .find(|item| Some(&item.id) == dep.service.as_ref())
                    .ok_or(DependencyError::VersionConflict)?;
                services.push(ServicePin {
                    consumer: consumer.clone(),
                    provider: provider.pin.clone(),
                    service: service.id.clone(),
                    version: service.version.clone(),
                });
                edges.push(DependencyEdge {
                    consumer: consumer.clone(),
                    provider: provider.pin.identity(),
                    service: service.id.clone(),
                });
                reverse
                    .get_mut(&provider.pin.identity())
                    .unwrap()
                    .insert(consumer.clone());
            }
        }
        services.sort();
        edges.sort();
        let mut seen = BTreeSet::new();
        let mut topology = Vec::new();
        for identity in selected.keys() {
            topological(identity, selected, &mut seen, &mut topology);
        }
        let mut lock = DependencyLock {
            format_version: 1,
            digest: String::new(),
            target: target.clone(),
            roots: roots.to_vec(),
            packages: selected
                .values()
                .map(|package| package.pin.clone())
                .collect(),
            services,
            tools: self.tool_pins(target, selected)?,
            edges,
            topology,
            reverse_dependencies: reverse
                .into_iter()
                .map(|(provider, consumers)| ReverseDependency {
                    provider,
                    consumers: consumers.into_iter().collect(),
                })
                .collect(),
        };
        lock.digest = lock.verified_digest()?;
        let plan = DependencyPlan {
            format_version: 1,
            mode: "plan-only".into(),
            lock,
        };
        if serde_json::to_vec(&plan)
            .map_err(|_| DependencyError::InvalidDeclaration)?
            .len()
            > crate::protocol::MAX_FRAME_BYTES - 16_384
        {
            return Err(DependencyError::BudgetExceeded);
        }
        Ok(plan)
    }
}
fn satisfies(
    package: &PackageCandidate,
    constraints: &[&ServiceDependency],
) -> Result<bool, DependencyError> {
    let candidate = version(&package.pin.version)?;
    for dep in constraints {
        if !range(&dep.version)?.satisfies(&candidate) {
            return Ok(false);
        }
        let (Some(service), Some(interface)) = (&dep.service, &dep.interface_version) else {
            return Err(DependencyError::InterfaceRequired);
        };
        let Some(definition) = package.services.iter().find(|item| item.id == *service) else {
            return Ok(false);
        };
        if !range(interface)?.satisfies(&version(&definition.version)?) {
            return Ok(false);
        }
    }
    Ok(true)
}
fn service_cycle(selected: &BTreeMap<DependencyIdentity, &PackageCandidate>) -> bool {
    fn visit(
        identity: &DependencyIdentity,
        selected: &BTreeMap<DependencyIdentity, &PackageCandidate>,
        active: &mut BTreeSet<DependencyIdentity>,
        done: &mut BTreeSet<DependencyIdentity>,
    ) -> bool {
        if done.contains(identity) {
            return false;
        }
        if !active.insert(identity.clone()) {
            return true;
        }
        if let Some(package) = selected.get(identity) {
            for dep in &package.service_dependencies {
                if selected.contains_key(&dep.identity())
                    && visit(&dep.identity(), selected, active, done)
                {
                    return true;
                }
            }
        }
        active.remove(identity);
        done.insert(identity.clone());
        false
    }
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    selected
        .keys()
        .any(|identity| visit(identity, selected, &mut active, &mut done))
}
fn topological(
    identity: &DependencyIdentity,
    selected: &BTreeMap<DependencyIdentity, &PackageCandidate>,
    seen: &mut BTreeSet<DependencyIdentity>,
    output: &mut Vec<DependencyIdentity>,
) {
    if !seen.insert(identity.clone()) {
        return;
    }
    let dependencies: BTreeSet<_> = selected[identity]
        .service_dependencies
        .iter()
        .map(ServiceDependency::identity)
        .collect();
    for dependency in dependencies {
        topological(&dependency, selected, seen, output);
    }
    output.push(identity.clone());
}
