-- 型注釈 + let 束縛
id :: a -> a ;
let id x = x;

-- 制約付き多相
square :: Num a => a -> a ;
let square x = x * x;

-- 複数束縛（セミコロン区切り）
let const x y = x; let one = 1;
