//! GSTR-1 table 9B (credit and debit notes, CDNR and CDNUR) JSON.

use super::b2b::{gstin_json, inv_type_code};
use super::emit::{
    amt, date_str, doc_number, flag, items_json, kv, map_arr, obj, pos_str, require_some, text,
};
use super::{CdnrParty, CdnurNote, CdnurType, NoteKind};
use crate::FormatError;

pub(super) fn note_code(k: NoteKind) -> &'static str {
    match k {
        NoteKind::Credit => "C",
        NoteKind::Debit => "D",
    }
}

pub(super) fn cdnr_json(list: &[CdnrParty]) -> Result<String, FormatError> {
    map_arr(list, |i, party| {
        let path = format!("cdnr[{i}]");
        require_some(&format!("{path}.nt"), &party.notes)?;
        let notes = map_arr(&party.notes, |j, n| {
            let p = format!("{path}.nt[{j}]");
            Ok(obj(&[
                kv("ntty", &text(note_code(n.kind))?),
                kv("nt_num", &doc_number(&format!("{p}.nt_num"), &n.num)?),
                kv("nt_dt", &date_str(n.date)),
                kv("val", &amt(n.value)),
                kv("pos", &pos_str(n.pos)),
                kv("rchrg", flag(n.reverse_charge)),
                kv("inv_typ", &text(inv_type_code(n.inv_type))?),
                kv("itms", &items_json(&format!("{p}.itms"), &n.items)?),
            ]))
        })?;
        Ok(obj(&[
            kv("ctin", &gstin_json(&party.ctin)?),
            kv("nt", &notes),
        ]))
    })
}

pub(super) fn cdnur_json(list: &[CdnurNote]) -> Result<String, FormatError> {
    map_arr(list, |i, n| {
        let p = format!("cdnur[{i}]");
        let mut f = Vec::new();
        match n.ur_type {
            CdnurType::B2cl { pos } => {
                f.push(kv("typ", "\"B2CL\""));
                f.push(kv("pos", &pos_str(pos)));
            }
            CdnurType::ExportWithPayment => f.push(kv("typ", "\"EXPWP\"")),
            CdnurType::ExportWithoutPayment => f.push(kv("typ", "\"EXPWOP\"")),
        }
        f.push(kv("ntty", &text(note_code(n.kind))?));
        f.push(kv("nt_num", &doc_number(&format!("{p}.nt_num"), &n.num)?));
        f.push(kv("nt_dt", &date_str(n.date)));
        f.push(kv("val", &amt(n.value)));
        f.push(kv("itms", &items_json(&format!("{p}.itms"), &n.items)?));
        Ok(obj(&f))
    })
}
