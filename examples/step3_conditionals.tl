-- パス: examples/step3_conditionals.tl
-- 役割: if 式と Bool 操作の基本例
-- 意図: 条件分岐と比較演算を紹介する
-- 関連ファイル: examples/basics.tl, README.md

absInt :: Int -> Int ;
let absInt n = if n < 0 then -n else n;

let isTeenager age =
  if age < 13 then False
  else if age > 19 then False
  else True;

let pickGreeting flag =
  if flag then "Hello, learner!" else "Keep practicing!";

