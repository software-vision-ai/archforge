//! The shared core metamodel (decision gm2.20): what every notation has in common. A project's one
//! root (`Model`), folders that organise elements of any notation, comments and tags on any
//! element, and trace links between elements of any notations, which make the notations
//! viewpoints of one model. Everything that refers to another notation is typed Any, so this crate
//! depends on no notation.
//!
//! The metamodel is `core.xmi`, generated into `generated/` by `cargo xtask generate core`; this
//! file holds the rules the MOF core cannot know and the helpers that keep them.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

mod generated;
pub mod skills;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use archforge_mof::{Invalid, MetaError, Metamodel, Obj, Store, Universe, Value};

pub use generated::classes::*;
pub use generated::meta::*;
pub use generated::schema::{JUNCTIONS, TABLES};
pub use generated::slabs::CoreSlabs;

/// A universe of the core and the metamodels given.
pub fn universe_with(others: Vec<Arc<Metamodel>>) -> Result<Arc<Universe>, MetaError> {
    let mut all = vec![metamodel()?];
    all.extend(others);
    Ok(Arc::new(Universe::new(all)?))
}

/// What the core's own rules report.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreProblem {
    /// Two tags of one name on one element.
    DuplicateTag { element: Obj, name: String, tags: [Obj; 2] },
    /// A link from an element to itself.
    SelfLink(Obj),
    /// A project has one Model and nothing else without a container.
    Roots(Vec<Obj>),
    /// A Model inside something: a project has one, at the root.
    NestedModel(Obj),
}

/// The core's rules over the whole store: tag names unique per element, no self-links, one root.
pub fn problems(store: &Store) -> Vec<CoreProblem> {
    let mut out = Vec::new();
    let mut tags_on: BTreeMap<(Obj, String), Obj> = BTreeMap::new();
    let mut roots = Vec::new();
    for o in store.alive() {
        let Ok(class) = store.class_of(o) else { continue };
        match store.container(o).ok().flatten() {
            None => roots.push(o),
            Some(_) if class == MODEL => out.push(CoreProblem::NestedModel(o)),
            Some(_) => {}
        }
        if class == TAG {
            let name = store.get(o, tag::NAME).ok().and_then(|v| v.as_str().map(str::to_lowercase)).unwrap_or_default();
            for e in store.list(o, tag::ELEMENT).unwrap_or_default().iter().filter_map(Value::as_ref) {
                if let Some(first) = tags_on.insert((e, name.clone()), o) {
                    out.push(CoreProblem::DuplicateTag { element: e, name: name.clone(), tags: [first, o] });
                }
            }
        }
        if class == LINK {
            let (s, t) = (store.get(o, link::SOURCE).ok().and_then(|v| v.as_ref()), store.get(o, link::TARGET).ok().and_then(|v| v.as_ref()));
            if s.is_some() && s == t {
                out.push(CoreProblem::SelfLink(o));
            }
        }
    }
    let one_model = roots.len() == 1 && roots.first().and_then(|r| store.class_of(*r).ok()) == Some(MODEL);
    if !roots.is_empty() && !one_model {
        out.push(CoreProblem::Roots(roots));
    }
    out
}

/// Whether a tag named `name` may be applied to `element`: no other tag of that name is on it.
pub fn tag_name_free(store: &Store, element: Obj, name: &str, except: Option<Obj>) -> bool {
    let wanted = name.to_lowercase();
    !store.referrers(element).into_iter().any(|(tag, prop)| {
        prop == tag::ELEMENT
            && Some(tag) != except
            && store.get(tag, tag::NAME).ok().and_then(|v| v.as_str().map(str::to_lowercase)).as_deref() == Some(wanted.as_str())
    })
}

/// A new link of `kind` from `source` to `target`, owned by `owner` (a Model). Refused: a link
/// from an element to itself, or with a link as an end. All or nothing.
pub fn link(store: &mut Store, owner: Obj, kind: LinkKind, source: Obj, target: Obj, note: Option<&str>) -> Result<Obj, Invalid> {
    if source == target {
        return Err(Invalid::Refused("a link cannot go from an element to itself".into()));
    }
    for end in [source, target] {
        if store.class_of(end)? == LINK {
            return Err(Invalid::Refused("a link cannot have a link as an end".into()));
        }
    }
    store.transaction(|st| {
        let l = st.create(LINK)?;
        st.set(l, link::KIND, Value::Enum(kind.index()))?;
        st.set(l, link::SOURCE, Value::Ref(source))?;
        st.set(l, link::TARGET, Value::Ref(target))?;
        if let Some(n) = note {
            st.set(l, link::NOTE, Value::str(n))?;
        }
        st.push(owner, model::LINK, Value::Ref(l))?;
        Ok(l)
    })
}

/// The links whose source or target is `obj`, anything it contains, or one of those links (a
/// link set through the store alone may point at a link), and the skills whose subject is one of
/// them: everything that has to go before `obj`, in the order it can go (a link pointing at a
/// link comes before the link it points at).
pub fn links_of(store: &Store, obj: Obj) -> Result<Vec<Obj>, Invalid> {
    let mut links = Vec::new();
    let mut found = BTreeSet::new();
    let mut stack = vec![obj];
    let mut seen = BTreeSet::new();
    while let Some(o) = stack.pop() {
        if !seen.insert(o) {
            continue;
        }
        for (src, prop) in store.referrers(o) {
            if (prop == link::SOURCE || prop == link::TARGET || prop == skill::SUBJECT) && found.insert(src) {
                links.push(src);
                stack.push(src);
            }
        }
        stack.extend(store.owned_elements(o)?);
    }
    links.reverse();
    Ok(links)
}

/// Destroys `obj` with everything it contains, and first every link that has one of them as an
/// end and every skill that sits on one of them. A link without its end is meaningless, and the
/// MOF core alone would refuse the destroy because a link's ends are required; a skill's subject
/// is optional, so without this cascade the MOF core would null it and a skill on one table would
/// silently apply to the whole model. Every notation removes through this function. All or
/// nothing. Gives how many objects went.
pub fn destroy_with_links(store: &mut Store, obj: Obj) -> Result<usize, Invalid> {
    let links = links_of(store, obj)?;
    store.transaction(|st| {
        let mut gone = 0;
        for l in links {
            if st.is_alive(l) && l != obj && !st.contains(obj, l) {
                gone += st.destroy(l)?;
            }
        }
        gone += st.destroy(obj)?;
        Ok(gone)
    })
}
