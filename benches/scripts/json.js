// JSON benchmark - tests JSON parse/stringify
var data = '{"name": "test", "value": 42, "items": [1, 2, 3, 4, 5]}';

var sum = 0;
for (var i = 0; i < 10000; i = i + 1) {
    var obj = JSON.parse(data);
    sum = sum + obj.value;
}
print("json sum = " + sum);
