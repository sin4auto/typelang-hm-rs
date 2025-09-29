-- パス: examples/step2_functions.tl
-- 役割: 関数定義と適用の基本例
-- 意図: 引数付き let 束縛と部分適用を学ぶ
-- 関連ファイル: examples/basics.tl, README.md

square :: Num a => a -> a;
let square x = x * x;

add :: Num a => a -> a -> a;
let add x y = x + y;

let addFive = add 5;
let twentyFive = square (addFive 0);
