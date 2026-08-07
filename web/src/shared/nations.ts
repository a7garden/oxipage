/** ISO-3166 alpha-2 → Korean display name. Ported from blog-test. */
export const NATION_CODE_TO_KO: Record<string, string> = {
  KR: "한국", US: "미국", JP: "일본", GB: "영국", FR: "프랑스", DE: "독일",
  CN: "중국", CA: "캐나다", AU: "호주", ES: "스페인", IT: "이탈리아",
  HK: "홍콩", TW: "대만", IN: "인도", RU: "러시아", SE: "스웨덴",
  DK: "덴마크", NL: "네덜란드", IE: "아일랜드", NZ: "뉴질랜드", MX: "멕시코",
  BR: "브라질", AR: "아르헨티나", BE: "벨기에", NO: "노르웨이", FI: "핀란드",
  PL: "폴란드", CH: "스위스", AT: "오스트리아", PT: "포르투갈", TR: "튀르키예",
  TH: "태국", ID: "인도네시아", PH: "필리핀", VN: "베트남", SG: "싱가포르",
  MY: "말레이시아", ZA: "남아프리카공화국", EG: "이집트", IL: "이스라엘",
  AE: "아랍에미리트", IR: "이란", IQ: "이라크", KZ: "카자흐스탄",
};

/** Split "KR,US" → ["KR","US"], trim, drop empties. */
export function parseNationCodes(origin: string | null | undefined): string[] {
  return (origin ?? "")
    .split(",")
    .map((c) => c.trim().toUpperCase())
    .filter((c) => c.length > 0);
}

/** Display name: known code → Korean, else the raw code. */
export function nationLabel(code: string): string {
  return NATION_CODE_TO_KO[code] ?? code;
}
