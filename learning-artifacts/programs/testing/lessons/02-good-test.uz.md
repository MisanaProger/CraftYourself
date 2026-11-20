# Sifatli test anatomiyasi

Yaxshi test faqat u tasvirlagan xatti-harakat buzilganda yiqiladi — va yiqilishi
aynan nima buzilganini darhol ko‘rsatadi.

## 1. Arrange — Act — Assert

Test uch qismdan iborat: ma’lumotni tayyorlash, bitta amalni bajarish, natijani tekshirish.

```rust
#[test]
fn discount_starts_at_exactly_1000() {
    // Arrange
    let cart = Cart::with_total(1000);
    // Act
    let price = cart.price_with_discount();
    // Assert
    assert_eq!(price, 950);
}
```

Testda bir nechta «Act» bo‘lsa, bu bir nechta test.

## 2. Bitta xatti-harakat — bitta test va tushunarli nom

Nom «nima rost bo‘lishi kerak?» savoliga javob beradi:
`discount_starts_at_exactly_1000`, `empty_input_returns_error` — `test1` yoki
`test_discount` emas. Bunday test yiqilganda nomi muammoni tushuntiradi.

## 3. Amalga oshirishni emas, xatti-harakatni tekshiring

Test tashqaridan ko‘rinadigan narsani tekshiradi: qaytgan qiymat, xato, holat
o‘zgarishi — qaysi yopiq funksiyalar chaqirilganini emas. Shunda test
refaktoringdan omon chiqadi va faqat haqiqiy buzilishda yiqiladi.

## 4. Deterministik va mustaqil

- Joriy vaqt, tasodifiy sonlar, tarmoq va bajarilish tartibi bo‘lmasin —
  ularni qotiring (berilgan vaqt, seed) yoki test chegarasidan chiqaring.
- Testlar bir-biriga va ishga tushirish tartibiga bog‘liq emas. Har biri o‘z
  ma’lumotini tayyorlaydi.

Ba’zan yiqiladigan «miltillovchi» test umuman yo‘qidan yomonroq: unga ishonmay qo‘yishadi.

## 5. Aniq tasdiqlar

`assert!(result.is_ok())` faqat «ok emas» deydi. `assert_eq!(result, Ok(950))`
kutilgan va haqiqiy qiymatni ko‘rsatadi. Aniq qiymatlarni solishtiring.

## 6. Chegaralar va ekvivalentlik sinflari

Kirish qiymatlari xatti-harakati bir xil bo‘lgan sinflarga bo‘linadi («summa
1000 dan kam — chegirmasiz»). Har sinfdan bitta qiymat yetarli, ammo **har bir
chegara** ikki tomondan tekshiriladi: 999 va 1000, 4999 va 5000. Ko‘p xatolar
aynan chegaralarda: `<=` o‘rniga `<`, bittaga adashish.

## 7. Regressiya testi: avval qizil

Xato topdingizmi? Avval uni takrorlaydigan va **yiqiladigan** test yozing.
Shundan keyingina kodni tuzating. Tuzatishdan keyingi yashil natija aynan shu
tuzatilganini isbotlaydi, test esa xatoning qaytishiga yo‘l qo‘ymaydi.

## Nimadan qochish kerak

- Testdagi mantiq (`if`, kutilgan qiymatni hisoblaydigan tsikllar).
- Amalga oshirishni testga ko‘chirish: test o‘sha xatoni takrorlaydi.
- Hammasini bitta ulkan testda tekshirish.
- Tasdiqsiz testlar.
