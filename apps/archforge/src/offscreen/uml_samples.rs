//! The screenshot tool's UML builders that only its pictures use: each stages one of UML's screens
//! through the narrow context, by UML's own steps (a qualifier's commit, a template parameter's
//! bin, the ends' editor and its refusal words, the stereotypes' values compartments) as UML's
//! screens list them for the tool.

use archforge_mof::Uuid;
use archforge_shell_api::cx::Ctx;
use archforge_uml_ui::snapshot::uml_state::{uml, uml_mut};
use archforge_uml_ui::snapshot::{uml_profiles, uml_qualifiers, uml_relate, uml_stereotypes, uml_templates};
use serde_json::json;

/// `typed` as a new qualifier's name, of type `ty`, on end `end` of association `association`,
/// applied and refused, as a sample to look at.
pub(super) fn refuse_sample(cx: &mut Ctx, association: Uuid, end: usize, typed: &str, ty: &str) {
    uml_qualifiers::open(cx, association, end, None);
    if let Some(e) = uml_mut(cx.notation_states).and_then(|uml| uml.qualifier.as_mut()) {
        e.name = typed.to_string();
        e.ty = ty.to_string();
        e.multiplicity = "1".to_string();
    }
    let Some(edit) = uml(cx.notation_states).and_then(|uml| uml.qualifier.clone()) else { return };
    uml_qualifiers::commit(cx, &edit);
}

/// The open template parameter editor's bin pressed, as a sample to look at: the removal refused
/// while a binding uses the parameter.
pub(super) fn remove_sample(cx: &mut Ctx) {
    if let Some(edit) = uml(cx.notation_states).and_then(|uml| uml.template.clone()) {
        uml_templates::remove_parameter(cx, &edit);
    }
}

/// `typed` in end `end`'s multiplicity of association `id`, committed and refused, as a sample
/// to look at.
pub(super) fn refuse_typed(cx: &mut Ctx, id: Uuid, end: usize, typed: &str) -> Result<(), String> {
    let store = cx.session.as_ref().ok_or("no project is open")?.doc();
    let a = store.by_uuid(id).ok_or("no such association")?;
    let mut edit = uml_relate::ends_edit(store, a, id).ok_or("not an association")?;
    let why = archforge_uml::members::parse_multiplicity(typed).err().ok_or("the sample reads")?;
    *edit.multiplicity.get_mut(end).ok_or("no such end")? = typed.to_string();
    *edit.refused.get_mut(end).ok_or("no such end")? = Some(uml_relate::multiplicity_problem(&why));
    if let Some(uml) = uml_mut(cx.notation_states) {
        uml.ends = Some(edit);
    }
    Ok(())
}

/// In the sample class model with Persistence applied and used (F6), the values of F3: Order's
/// table set to `orders`, cached left to its default; the profile Auditing (Auditable, extending
/// Class) applied to Commerce, and Logging (Tracked, extending Class) applied nowhere, so the picker
/// (F4) shows each kind of row; the open diagram showing the values. Gives Order's id.
pub(super) fn put_values(cx: &mut Ctx) -> Result<Uuid, String> {
    let (commerce, _, entity) = uml_profiles::put_profiles(cx, true)?;
    let session = cx.session.as_mut().ok_or("the sample needs a session")?;
    let order = {
        let st = session.doc();
        let o =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Order").ok_or("Order")?;
        st.uuid(o).map_err(|e| e.to_string())?
    };
    let id = |v: serde_json::Value| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
    let mut run = |cmd: &str, p: serde_json::Value| session.execute(cmd, &p).map(|o| id(o.value)).map_err(|e| format!("{cmd}: {e}"));
    run("uml.stereotype.set-value", json!({ "element": order.to_string(), "stereotype": entity, "property": "table", "values": ["orders"] }))?;
    let auditing = run("uml.profile.add", json!({ "name": "Auditing" }))?;
    run("uml.stereotype.add", json!({ "profile": auditing, "name": "Auditable", "extends": ["Class"] }))?;
    run("uml.profile.apply", json!({ "package": commerce, "profile": auditing }))?;
    let logging = run("uml.profile.add", json!({ "name": "Logging" }))?;
    run("uml.stereotype.add", json!({ "profile": logging, "name": "Tracked", "extends": ["Class"] }))?;
    if let Some(d) = cx.canvas.active {
        uml_stereotypes::show_values(cx, d, true)?;
    }
    if let Some(s) = cx.session.as_mut() {
        s.mark_saved().map_err(|e| e.to_string())?;
    }
    Ok(order)
}
