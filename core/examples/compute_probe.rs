//! 计算拆分探针（Rust 侧，无界面）：cargo run --release -p bench-core --example compute_probe
//! 与 compute 场景同数据、同算法；把耗时拆成 复制 / 排序 / 解析为 Value 或强类型，并校验结果。

use std::hint::black_box;
use std::time::Instant;

use bench_core::{Item, closes, items, task_agg, task_ema, task_json, task_sort};

fn measure<T>(name: &str, n: usize, mut f: impl FnMut() -> T) {
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        black_box(f());
        v.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let first = v[0];
    let mut s = v[1..].to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{name:24} first {first:8.3} ms  p50 {:8.3} ms  p95 {:8.3} ms", s[s.len() / 2], s[s.len() * 95 / 100]);
}

fn main() {
    let items = items(100_000);
    let closes = closes(100_000);
    let json = bench_core::big_json(33_000);
    let chg: Vec<f64> = items.iter().map(|i| i.chg_pct).collect();

    // 校验：排序后降序；聚合与逐项求和一致；JSON 元素数
    let mut v: Vec<&Item> = items.iter().collect();
    v.sort_by(|a, b| b.chg_pct.partial_cmp(&a.chg_pct).unwrap());
    assert!(v.windows(2).all(|w| w[0].chg_pct >= w[1].chg_pct));
    let naive: f64 = items.iter().map(|i| i.price * i.qty + (i.price - i.cost) * i.qty).sum();
    assert!((task_agg(&items) - naive).abs() < 1e-6 * naive.abs());
    assert_eq!(task_json(&json), 33_000);
    println!("checks ok; json {:.2} MB", json.len() as f64 / 1048576.0);

    let n = 31;
    measure("sort.full", n, || task_sort(&items));
    measure("sort.copy_refs", n, || items.iter().collect::<Vec<&Item>>());
    measure("sort.cmp_only", n, || {
        let mut v: Vec<&Item> = items.iter().collect();
        v.sort_by(|a, b| b.chg_pct.partial_cmp(&a.chg_pct).unwrap());
        v.len()
    });
    measure("sort.f64", n, || {
        let mut v = chg.clone();
        v.sort_by(|a, b| b.partial_cmp(a).unwrap());
        v[0]
    });
    measure("agg", n, || task_agg(&items));
    measure("ema", n, || task_ema(&closes));
    measure("json.value", n, || task_json(&json));
    measure("json.typed", n, || serde_json::from_str::<Vec<Item>>(&json).unwrap().len());
    // Value 树的释放单独计时（task_json 的耗时含解析与释放）
    let mut drops: Vec<f64> = (0..n)
        .map(|_| {
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            let t = Instant::now();
            drop(black_box(v));
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    drops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{:24} p50 {:8.3} ms", "json.value_drop", drops[n / 2]);
    let f0 = bench_core::thread_minflt();
    black_box(task_json(&json));
    println!("{:24} minflt {}", "json.value", bench_core::thread_minflt() - f0);
    // 低频执行（每 300ms 一次，与基准节奏相同）：缓存变冷、CPU 降频后的同一计算
    for name in ["agg", "sort.full", "json.value"] {
        let mut v: Vec<f64> = (0..15)
            .map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                let t = Instant::now();
                match name {
                    "agg" => drop(black_box(task_agg(&items))),
                    "sort.full" => drop(black_box(task_sort(&items))),
                    _ => drop(black_box(task_json(&json))),
                }
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("{:24} p50 {:8.3} ms (每 300ms 一次)", format!("{name}.sparse"), v[7]);
    }
}
