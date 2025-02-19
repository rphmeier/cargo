use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use core::PackageId;
use core::registry::PackageRegistry;
use core::{Resolve, SourceId, Package};
use core::resolver::Method;
use ops;
use util::config::Config;
use util::CargoResult;

pub struct UpdateOptions<'a> {
    pub config: &'a Config,
    pub to_update: &'a [String],
    pub precise: Option<&'a str>,
    pub aggressive: bool,
}

pub fn generate_lockfile(manifest_path: &Path, config: &Config)
                         -> CargoResult<()> {
    let package = try!(Package::for_path(manifest_path, config));
    let mut registry = PackageRegistry::new(config);
    let resolve = try!(ops::resolve_with_previous(&mut registry, &package,
                                                  Method::Everything,
                                                  None, None));
    try!(ops::write_pkg_lockfile(&package, &resolve));
    Ok(())
}

pub fn update_lockfile(manifest_path: &Path,
                       opts: &UpdateOptions) -> CargoResult<()> {

    if opts.aggressive && opts.precise.is_some() {
        bail!("cannot specify both aggressive and precise simultaneously")
    }

    let package = try!(Package::for_path(manifest_path, opts.config));

    let previous_resolve = match try!(ops::load_pkg_lockfile(&package, opts.config)) {
    	Some(resolve) => resolve,
	None => {
	     return generate_lockfile(manifest_path, opts.config);
	}
    };
    let mut registry = PackageRegistry::new(opts.config);
    let mut to_avoid = HashSet::new();

    if opts.to_update.is_empty() {
        to_avoid.extend(previous_resolve.iter());
    } else {
        let mut sources = Vec::new();
        for name in opts.to_update {
            let dep = try!(previous_resolve.query(name));
            if opts.aggressive {
                fill_with_deps(&previous_resolve, dep, &mut to_avoid,
                               &mut HashSet::new());
            } else {
                to_avoid.insert(dep);
                sources.push(match opts.precise {
                    Some(precise) => {
                        // TODO: see comment in `resolve.rs` as well, but this
                        //       seems like a pretty hokey reason to single out
                        //       the registry as well.
                        let precise = if dep.source_id().is_registry() {
                            format!("{}={}", dep.name(), precise)
                        } else {
                            precise.to_string()
                        };
                        dep.source_id().clone().with_precise(Some(precise))
                    }
                    None => {
                        dep.source_id().clone().with_precise(None)
                    }
                });
            }
        }
        try!(registry.add_sources(&sources));
    }

    let resolve = try!(ops::resolve_with_previous(&mut registry,
                                                  &package,
                                                  Method::Everything,
                                                  Some(&previous_resolve),
                                                  Some(&to_avoid)));

    // Summarize what is changing for the user.
    let print_change = |status: &str, msg: String| {
        opts.config.shell().status(status, msg)
    };
    for (removed, added) in compare_dependency_graphs(&previous_resolve, &resolve) {
        if removed.len() == 1 && added.len() == 1 {
            let msg = if removed[0].source_id().is_git() {
                format!("{} -> #{}", removed[0],
                        &added[0].source_id().precise().unwrap()[..8])
            } else {
                format!("{} -> v{}", removed[0], added[0].version())
            };
            try!(print_change("Updating", msg));
        } else {
            for package in removed.iter() {
                try!(print_change("Removing", format!("{}", package)));
            }
            for package in added.iter() {
                try!(print_change("Adding", format!("{}", package)));
            }
        }
    }

    try!(ops::write_pkg_lockfile(&package, &resolve));
    return Ok(());

    fn fill_with_deps<'a>(resolve: &'a Resolve, dep: &'a PackageId,
                          set: &mut HashSet<&'a PackageId>,
                          visited: &mut HashSet<&'a PackageId>) {
        if !visited.insert(dep) {
            return
        }
        set.insert(dep);
        if let Some(deps) =  resolve.deps(dep) {
            for dep in deps {
                fill_with_deps(resolve, dep, set, visited);
            }
        }
    }

    fn compare_dependency_graphs<'a>(previous_resolve: &'a Resolve,
                                     resolve: &'a Resolve) ->
                                     Vec<(Vec<&'a PackageId>, Vec<&'a PackageId>)> {
        fn key(dep: &PackageId) -> (&str, &SourceId) {
            (dep.name(), dep.source_id())
        }

        // Removes all package ids in `b` from `a`. Note that this is somewhat
        // more complicated because the equality for source ids does not take
        // precise versions into account (e.g. git shas), but we want to take
        // that into account here.
        fn vec_subtract<'a>(a: &[&'a PackageId],
                            b: &[&'a PackageId]) -> Vec<&'a PackageId> {
            a.iter().filter(|a| {
                // If this package id is not found in `b`, then it's definitely
                // in the subtracted set
                let i = match b.binary_search(a) {
                    Ok(i) => i,
                    Err(..) => return true,
                };

                // If we've found `a` in `b`, then we iterate over all instances
                // (we know `b` is sorted) and see if they all have different
                // precise versions. If so, then `a` isn't actually in `b` so
                // we'll let it through.
                //
                // Note that we only check this for non-registry sources,
                // however, as registries countain enough version information in
                // the package id to disambiguate
                if a.source_id().is_registry() {
                    return false
                }
                b[i..].iter().take_while(|b| a == b).all(|b| {
                    a.source_id().precise() != b.source_id().precise()
                })
            }).cloned().collect()
        }

        // Map (package name, package source) to (removed versions, added versions).
        let mut changes = BTreeMap::new();
        let empty = (Vec::new(), Vec::new());
        for dep in previous_resolve.iter() {
            changes.entry(key(dep)).or_insert(empty.clone()).0.push(dep);
        }
        for dep in resolve.iter() {
            changes.entry(key(dep)).or_insert(empty.clone()).1.push(dep);
        }

        for (_, v) in changes.iter_mut() {
            let (ref mut old, ref mut new) = *v;
            old.sort();
            new.sort();
            let removed = vec_subtract(old, new);
            let added = vec_subtract(new, old);
            *old = removed;
            *new = added;
        }
        debug!("{:#?}", changes);

        changes.into_iter().map(|(_, v)| v).collect()
    }
}
