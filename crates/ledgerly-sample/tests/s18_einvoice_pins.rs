//! Round s18: the sample company is e-invoice ready. Its seed gives the company and every
//! registered party a two-line address and a PIN code valid for the stated city and state, so the
//! sales invoices export as e-invoice JSON (INV-01) instead of "the company has no PIN code".
//!
//! Provenance: derived. The values below are the seed's own addresses (`assets/godavari.toml`,
//! written by hand: synthetic street lines, PIN codes from the India Post ranges of each city:
//! Pune 411xxx, Coimbatore 641xxx, Nashik 422xxx) and the NIC e-invoice schema page
//! (<https://einv-apisandbox.nic.in/version1.03/generate-irn.html>, read 2026-11-26): the first
//! address line is `Addr1`, the last is `Loc`, `Pin` is a 6-digit number, `Stcd` is the 2-digit
//! state code, `Pos` the place of supply. SI/0405 (6 Nov 2026) is 60 hand sprayers (HSN 8424, unit
//! NOS) at Rs 1,450.00 to Lakshmi Farm Supplies, Coimbatore, Tamil Nadu (33), from Maharashtra (27):
//! inter-state, so IGST at 5% on 60 x 1,450.00 = 87,000.00 is 4,350.00 and the invoice value is
//! 91,350.00 (the sample's own tests derive the same 4,350.00 on SI/0405).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop on any failure"
)]

use ledgerly_core::{Clock, Outcome};
use time::OffsetDateTime;

fn json_of(number: &str) -> serde_json::Value {
    let dir = tempfile::tempdir().unwrap();
    let api = ledgerly_sample::create(
        &dir.path().join("s.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("books")
    };
    let id = books
        .vouchers
        .values()
        .find(|v| v.number.as_deref() == Some(number))
        .unwrap()
        .id;
    match api.einvoice_json(id).unwrap() {
        Outcome::Value(json) => serde_json::from_str(&json).unwrap(),
        Outcome::Unsupported(u) => panic!("not exportable: {} ({})", u.what, u.reason),
    }
}

#[test]
fn si_0405_exports_with_the_sellers_and_buyers_street_place_and_pin() {
    let v = json_of("SI/0405");
    assert_eq!(v["DocDtls"]["No"], "SI/0405");
    assert_eq!(v["DocDtls"]["Dt"], "06/11/2026");
    // Seller: the company, Pune, Maharashtra (27).
    assert_eq!(v["SellerDtls"]["Gstin"], "27ZZZZZ0000Z1ZQ");
    assert_eq!(
        v["SellerDtls"]["Addr1"],
        "Plot 14, Sample Industrial Estate"
    );
    assert_eq!(v["SellerDtls"]["Loc"], "Pune");
    assert_eq!(v["SellerDtls"]["Pin"], 411_026);
    assert_eq!(v["SellerDtls"]["Stcd"], "27");
    // Buyer: Lakshmi Farm Supplies, Coimbatore, Tamil Nadu (33).
    assert_eq!(v["BuyerDtls"]["Gstin"], "33ZZZZZ0007Z1ZQ");
    assert_eq!(v["BuyerDtls"]["Addr1"], "Survey 88, Sample Farm Road");
    assert_eq!(v["BuyerDtls"]["Loc"], "Coimbatore");
    assert_eq!(v["BuyerDtls"]["Pin"], 641_012);
    assert_eq!(v["BuyerDtls"]["Stcd"], "33");
    assert_eq!(v["BuyerDtls"]["Pos"], "33");
    // The item and totals: 60 x 1,450.00 = 87,000.00, IGST 5% = 4,350.00, total 91,350.00.
    let item = &v["ItemList"][0];
    assert_eq!(item["HsnCd"], "8424");
    assert_eq!(item["Unit"], "NOS");
    assert_eq!(item["Qty"], 60.0);
    assert_eq!(item["UnitPrice"], 1450.0);
    assert_eq!(item["AssAmt"], 87_000.0);
    assert_eq!(item["IgstAmt"], 4_350.0);
    assert_eq!(item["TotItemVal"], 91_350.0);
    assert_eq!(v["ValDtls"]["TotInvVal"], 91_350.0);
}

#[test]
fn every_registered_party_has_a_two_line_address_and_a_pin_of_its_own_state() {
    // PIN first digits by state (India Post circles): Maharashtra 4, Karnataka 5, Tamil Nadu 6,
    // Madhya Pradesh 4, Gujarat 3. The first three digits name the sorting district: Pune 411,
    // Nashik 422, Satara 415, Mysuru 570, Bengaluru 560, Ahmedabad 382, Rajkot 360, Coimbatore 641,
    // Indore 452.
    let dir = tempfile::tempdir().unwrap();
    let api = ledgerly_sample::create(
        &dir.path().join("s.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("books")
    };
    let want = [
        ("Kaveri Packaging Pvt Ltd", "Mysuru", "570"),
        ("Nilgiri Steel Traders", "Bengaluru", "560"),
        ("Vardhaman Logistics", "Pune", "411"),
        ("Rupa Polymers", "Ahmedabad", "382"),
        ("Shree Ganesh Traders", "Nashik", "422"),
        ("Mehta Agro Industries", "Rajkot", "360"),
        ("Lakshmi Farm Supplies", "Coimbatore", "641"),
        ("Patil Hardware Stores", "Satara", "415"),
        ("Kisan Seva Kendra", "Indore", "452"),
    ];
    for (name, city, pin_prefix) in want {
        let party = books
            .masters
            .ledgers
            .values()
            .find(|l| l.name == name)
            .and_then(|l| l.party.as_ref())
            .unwrap();
        assert_eq!(party.address.lines.len(), 2, "{name}");
        assert_eq!(party.address.lines[1], city, "{name}");
        let pin = party.address.pincode.as_deref().unwrap();
        assert_eq!(pin.len(), 6, "{name}");
        assert!(pin.starts_with(pin_prefix), "{name}: {pin}");
    }
    assert_eq!(books.company.address.pincode.as_deref(), Some("411026"));
}
