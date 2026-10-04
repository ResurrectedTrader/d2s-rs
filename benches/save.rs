use criterion::{black_box, criterion_group, criterion_main, Criterion};
use d2s::overlay::D2SaveLayout;
use d2s::{D2Save, TxtData};

fn benchmarks(c: &mut Criterion) {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/resources/99/Roka.d2s")).unwrap();
    let data = TxtData::embedded();
    let save = D2Save::from_bytes(&bytes).unwrap();
    let mut buf = Vec::with_capacity(save.estimate_size());

    c.bench_function("deserialize", |b| b.iter(|| D2Save::from_bytes_with(black_box(&bytes), data).unwrap()));
    c.bench_function("serialize", |b| {
        b.iter(|| {
            buf.clear();
            save.write_to(&mut buf, data).unwrap()
        })
    });
    c.bench_function("round_trip", |b| {
        b.iter(|| {
            let save = D2Save::from_bytes_with(black_box(&bytes), data).unwrap();
            buf.clear();
            save.write_to(&mut buf, data).unwrap()
        })
    });
    c.bench_function("overlay_read_name", |b| {
        b.iter(|| D2SaveLayout::new(black_box(&bytes[..])).unwrap().name().len())
    });
    c.bench_function("overlay_modify_and_checksum", |b| {
        b.iter(|| {
            let mut copy = bytes.clone();
            let mut layout = D2SaveLayout::new(&mut copy[..]).unwrap();
            layout.set_name("TestName").unwrap();
            layout.character_mut().set_level(99);
            layout.update_checksum();
            copy
        })
    });
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
