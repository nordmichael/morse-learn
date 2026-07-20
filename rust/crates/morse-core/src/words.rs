// Practice word pool for the trainer.
//
// This list is generated, not hand-copied. It is the most common English words
// (by frequency) that satisfy the trainer's constraints — lowercase a-z, 2-8
// letters, real dictionary words, nothing offensive — grouped so every letter
// in the learning order has words that use only the letters introduced up to
// that point. That guarantees the letter-progression always has words to draw
// from. It is an original selection built from a public word-frequency list and
// an English dictionary, not from the original game's word file.

/// All practice words, ordered by which letter each one introduces.
pub const WORDS: &[&str] = &[
    "tee", "at", "eat", "tea", "ate", "tat", "teat", "it",
    "tie", "tit", "me", "time", "am", "team", "meet", "met",
    "ma", "meat", "item", "mate", "aim", "mama", "mat", "tame",
    "mitt", "emit", "mite", "mime", "is", "as", "its", "see",
    "same", "state", "set", "miss", "site", "test", "east", "seem",
    "sea", "sit", "mass", "estate", "seat", "taste", "sam", "sat",
    "to", "so", "some", "most", "too", "tom", "mo", "sometime",
    "tattoo", "os", "toe", "moses", "toast", "toss", "moss", "tomato",
    "atom", "motto", "moist", "otto", "the", "that", "this", "he",
    "his", "has", "she", "them", "him", "these", "those", "home",
    "oh", "hit", "shot", "hot", "hate", "smith", "heat", "host",
    "in", "on", "not", "an", "one", "no", "than", "into",
    "then", "man", "name", "mean", "can", "come", "since", "each",
    "case", "once", "came", "nice", "comes", "act", "chance", "action",
    "are", "or", "more", "their", "there", "her", "other", "here",
    "three", "another", "care", "start", "and", "do", "had", "said",
    "need", "did", "made", "does", "end", "second", "done", "read",
    "out", "our", "us", "much", "such", "use", "around", "under",
    "house", "must", "sure", "music", "make", "think", "take", "took",
    "thank", "kind", "ask", "taken", "check", "market", "attack", "mark",
    "all", "like", "also", "could", "should", "still", "last", "look",
    "little", "old", "school", "lot", "of", "for", "from", "if",
    "after", "first", "off", "life", "find", "found", "feel", "free",
    "be", "but", "about", "been", "back", "because", "before", "best",
    "both", "better", "number", "business", "up", "people", "part", "help",
    "place", "keep", "please", "put", "public", "top", "person", "point",
    "get", "good", "go", "right", "being", "going", "got", "great",
    "through", "long", "high", "during", "just", "job", "john", "major",
    "project", "june", "james", "join", "jesus", "justice", "subject", "judge",
    "have", "over", "even", "never", "love", "give", "ever", "having",
    "live", "believe", "service", "five", "question", "quite", "quick", "queen",
    "square", "require", "request", "unique", "quarter", "squad", "equal", "quiet",
    "with", "was", "we", "will", "what", "when", "were", "who",
    "which", "would", "new", "how", "next", "six", "example", "tax",
    "box", "except", "expect", "text", "extra", "explain", "texas", "complex",
    "you", "your", "they", "only", "any", "way", "very", "may",
    "really", "year", "day", "many", "size", "amazing", "crazy", "realize",
    "magazine", "zone", "zero", "prize", "amazon", "citizen", "dozen", "frozen",
];
