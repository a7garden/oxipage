-- 공통 전문 검색 인덱스 (doc/01 §1.7, doc/02 §2.13)
-- trigram 토크나이저: 한국어 등 CJK에서 부분문자열 매칭 품질 확보
-- FTS5 column 정의는 column-name [type] [UNINDEXED] 순서. NOT NULL은 의미 없으므로 생략.
CREATE VIRTUAL TABLE IF NOT EXISTS search_documents USING fts5(
    extension_id UNINDEXED,
    doc_id       UNINDEXED,
    title,
    body,
    lang         UNINDEXED,
    published_at UNINDEXED,
    tokenize = 'trigram'
);
