use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mquickjs::Context;

fn bench_fib(c: &mut Criterion) {
    // Use iterative fibonacci to avoid stack overflow
    let code = r#"
        function fib(n) {
            if (n <= 1) return n;
            var a = 0;
            var b = 1;
            for (var i = 2; i <= n; i = i + 1) {
                var c = a + b;
                a = b;
                b = c;
            }
            return b;
        }
        var sum = 0;
        for (var i = 0; i < 1000; i = i + 1) {
            sum = sum + fib(30);
        }
        return sum;
    "#;

    c.bench_function("fib_iter 1k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(64 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_loop(c: &mut Criterion) {
    let code = r#"
        var sum = 0;
        for (var i = 0; i < 10000; i = i + 1) {
            sum = sum + i;
        }
        return sum;
    "#;

    c.bench_function("loop 10k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(64 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_array_push(c: &mut Criterion) {
    let code = r#"
        var arr = [];
        for (var i = 0; i < 10000; i = i + 1) {
            arr.push(i);
        }
        return arr.length;
    "#;

    c.bench_function("array push 10k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(256 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_object_create(c: &mut Criterion) {
    let code = r#"
        function Point(x, y) {
            this.x = x;
            this.y = y;
        }
        var points = [];
        for (var i = 0; i < 1000; i = i + 1) {
            points.push(new Point(i, i * 2));
        }
        return points.length;
    "#;

    c.bench_function("object create 1k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(256 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_closure(c: &mut Criterion) {
    let code = r#"
        function makeAdder(x) {
            function adder(y) {
                return x + y;
            }
            return adder;
        }
        var sum = 0;
        for (var i = 0; i < 1000; i = i + 1) {
            var add = makeAdder(i);
            sum = sum + add(i);
        }
        return sum;
    "#;

    c.bench_function("closure 1k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(256 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_string_concat(c: &mut Criterion) {
    let code = r#"
        var s = "";
        for (var i = 0; i < 1000; i = i + 1) {
            s = s + "x";
        }
        return s.length;
    "#;

    c.bench_function("string concat 1k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(64 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_json_parse(c: &mut Criterion) {
    let code = r#"
        var data = '{"name": "test", "value": 42, "items": [1, 2, 3]}';
        var sum = 0;
        for (var i = 0; i < 1000; i = i + 1) {
            var obj = JSON.parse(data);
            sum = sum + obj.value;
        }
        return sum;
    "#;

    c.bench_function("json parse 1k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(256 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_sieve(c: &mut Criterion) {
    let code = r#"
        function sieve(n) {
            var primes = [];
            for (var i = 0; i <= n; i = i + 1) {
                primes.push(true);
            }
            primes[0] = false;
            primes[1] = false;
            for (var i = 2; i * i <= n; i = i + 1) {
                if (primes[i]) {
                    for (var j = i * i; j <= n; j = j + i) {
                        primes[j] = false;
                    }
                }
            }
            var count = 0;
            for (var i = 0; i <= n; i = i + 1) {
                if (primes[i]) count = count + 1;
            }
            return count;
        }
        return sieve(10000);
    "#;

    c.bench_function("sieve 10k", |b| {
        b.iter(|| {
            let mut ctx = Context::new(256 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

fn bench_recursion(c: &mut Criterion) {
    // Benchmark recursion with a simpler function
    let code = r#"
        function sum(n) {
            if (n <= 0) return 0;
            return n + sum(n - 1);
        }
        var total = 0;
        for (var i = 0; i < 100; i = i + 1) {
            total = total + sum(100);
        }
        return total;
    "#;

    c.bench_function("recursion 100x100", |b| {
        b.iter(|| {
            let mut ctx = Context::new(128 * 1024);
            black_box(ctx.eval(code).unwrap())
        })
    });
}

criterion_group!(
    benches,
    bench_fib,
    bench_loop,
    bench_array_push,
    bench_object_create,
    bench_closure,
    bench_string_concat,
    bench_json_parse,
    bench_sieve,
    bench_recursion,
);

criterion_main!(benches);
