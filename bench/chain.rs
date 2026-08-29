fn main() {
    let n: i64 = std::env::args().nth(1).unwrap().parse().unwrap();
    let mut sum: i64 = 0;
    let mut i: i64 = 1;
    while i <= n { sum = (sum + i) % 1_000_000_007; i += 1; }
    println!("{}", std::hint::black_box(sum));
}
