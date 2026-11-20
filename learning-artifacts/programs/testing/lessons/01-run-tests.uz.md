# Testlarni ishga tushirish va natijani o‘qish

Testlar kod siz o‘ylagan ishni qilayotganini bilishning eng tez yo‘li. Ammo
kerakli qismni ishga tushira olmasangiz va aynan nima buzilganini tushunmasangiz,
ulardan foyda kam.

## Hammasi, bir qismi, bitta test

Har bir ekotizim barcha testlarni ishga tushira oladi va doirani toraytira oladi:

| Maqsad | Rust (cargo) | Python (pytest) | C/C++ (CTest) |
|---|---|---|---|
| Barcha testlar | `cargo test` | `python -m pytest` | `ctest --test-dir build` |
| Bitta paket / modul | `cargo test -p globset` | `python -m pytest tests/test_parser.py` | `ctest -R parser` |
| Test nomi bo‘yicha | `cargo test parse_range` | `python -m pytest -k parse_range` | `ctest -R '^parse_range$'` |
| `println!`/`print` chiqishini ko‘rsatish | `cargo test -- --nocapture` | `python -m pytest -s` | `ctest --output-on-failure` |
| Birinchi xatoda to‘xtash | `cargo test -- --test-threads=1`, birinchi xatoni o‘qing | `python -m pytest -x` | `ctest --stop-on-failure` |

Nom bo‘yicha filtr — debuggingdagi asosiy usul: muammoni takrorlaydigan testni
ishga tushirasiz va javobni daqiqalar emas, soniyalarda olasiz.

Craft Yourself’da testlar **sandboxda** ishlaydi: loyiha kodi tarmoqsiz,
xotira va vaqt cheklovlari bilan izolyatsiyalangan konteynerda bajariladi va
ish katalogingizdagi fayllarni o‘zgartira olmaydi. Tests yorlig‘idagi
«Testlarni ishga tushirish» tugmasi va `est test` buyrug‘i bir xil ish qiladi.

## Natijani o‘qish

`cargo test` ning odatiy natijasi:

```text
running 3 tests
test tests::parses_simple_range ... ok
test tests::parses_inclusive_range ... FAILED
test tests::rejects_garbage ... ok

failures:

---- tests::parses_inclusive_range stdout ----
thread 'tests::parses_inclusive_range' panicked at src/lib.rs:21:9:
assertion `left == right` failed
  left: [1, 2]
 right: [1, 2, 3]

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Uni shu tartibda o‘qing:

1. **Yakuniy satr** (`test result`) — nechtasi o‘tdi, yiqildi, o‘tkazib yuborildi.
2. **Yiqilgan testning nomi** — qaysi xatti-harakat buzilgan.
3. **Joy** (`src/lib.rs:21:9`) — xato emas, tekshiruv satri.
4. **Kutilgan va haqiqiy qiymat** — `left` (olingan) va `right` (kutilgan).
   Ular orasidagi farq — birinchi ishorangiz.

**Kompilyatsiya** xatosi yiqilgan test emas: testlar umuman ishlamagan.
Avval yig‘ishni tuzating.

## Yashil — hali «to‘g‘ri» degani emas

Barcha testlar o‘tsa, faqat tekshirilgan narsa ishlashini bilasiz. Hech narsani
tasdiqlamaydigan test doim yashil. Shuning uchun testning **yiqila olishini**
bir marta tekshirish odat bo‘lsin: kodni yoki kutilgan qiymatni vaqtincha
buzing va test buni sezganini ko‘ring.

## Sinab ko‘ring

1. Loyihada testlar qanday ishga tushirilishini toping (Tests yorlig‘i buyruqni ko‘rsatadi).
2. Barcha testlarni, keyin nomi bo‘yicha bittasini ishga tushiring.
3. Istalgan testda kutilgan qiymatni vaqtincha o‘zgartiring, ishga tushiring va
   natijani yuqoridagi tartibda o‘qing. Keyin qaytaring.
