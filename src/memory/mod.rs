pub mod db;

pub use db::{
    EpisodeSummary, MemoryMaintenanceReport, MemoryMessage, MemoryStats, MemoryStore,
    SemanticFact,
};

use crate::config::MemoryConfig;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

pub trait MemoryEmbedder: Send + Sync {
    fn id(&self) -> &str;
    fn embed(&self, text: &str) -> Vec<f32>;
}

#[derive(Debug)]
pub struct HashEmbedder {
    dimensions: usize,
    id: String,
}

impl HashEmbedder {
    pub fn new(dimensions: usize) -> Self {
        let dimensions = dimensions.clamp(64, 4096);
        Self {
            dimensions,
            id: format!("local-hash-v1-{dimensions}"),
        }
    }
}

impl Default for HashEmbedder {
    fn default() -> Self {
        Self::new(384)
    }
}

impl MemoryEmbedder for HashEmbedder {
    fn id(&self) -> &str {
        &self.id
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0_f32; self.dimensions];
        let tokens = memory_tokens(text);
        for token in tokens {
            let mut hasher = DefaultHasher::new();
            token.hash(&mut hasher);
            let hash = hasher.finish();
            let index = (hash as usize) % self.dimensions;
            let sign = if (hash >> 63) == 0 { 1.0 } else { -1.0 };
            vector[index] += sign;
        }
        normalize_vector(&mut vector);
        vector
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryContext {
    pub recent_messages: Vec<MemoryMessage>,
    pub facts: Vec<SemanticFact>,
    pub long_term_text: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConsolidationOutput {
    pub summary: String,
    pub facts: Vec<(String, f32, Option<i64>)>,
}

pub trait MemoryConsolidator: Send + Sync {
    fn consolidate(&self, messages: &[MemoryMessage]) -> ConsolidationOutput;
}

#[derive(Debug, Default)]
pub struct HeuristicConsolidator;

impl MemoryConsolidator for HeuristicConsolidator {
    fn consolidate(&self, messages: &[MemoryMessage]) -> ConsolidationOutput {
        let mut user_points = Vec::new();
        let mut character_points = Vec::new();
        let mut facts = Vec::new();

        for message in messages {
            let condensed = condense(&message.content, 240);
            match message.role.as_str() {
                "user" => {
                    if !condensed.is_empty() {
                        user_points.push(condensed);
                    }
                    for fact in extract_fact_candidates(&message.content, message.id) {
                        facts.push(fact);
                    }
                }
                "character" | "assistant" => {
                    if !condensed.is_empty() {
                        character_points.push(condensed);
                    }
                }
                _ => {}
            }
        }

        let mut summary = String::from("对话摘要：");
        if !user_points.is_empty() {
            summary.push_str("用户提到：");
            summary.push_str(&user_points.join("；"));
            summary.push('。');
        }
        if !character_points.is_empty() {
            summary.push_str("角色回应：");
            summary.push_str(&character_points.join("；"));
            summary.push('。');
        }

        ConsolidationOutput {
            summary: condense(&summary, 2200),
            facts,
        }
    }
}

#[derive(Clone)]
pub struct MemoryService {
    store: MemoryStore,
    embedder: Arc<dyn MemoryEmbedder>,
    consolidator: Arc<dyn MemoryConsolidator>,
}

impl MemoryService {
    pub fn open<P: AsRef<Path>>(
        directory: P,
        character_name: &str,
        embedding_dimensions: usize,
    ) -> rusqlite::Result<Self> {
        Ok(Self {
            store: MemoryStore::for_character_in_dir(directory, character_name)?,
            embedder: Arc::new(HashEmbedder::new(embedding_dimensions)),
            consolidator: Arc::new(HeuristicConsolidator),
        })
    }

    /// Injection point for replacing the default embedder or consolidator.
    #[allow(dead_code)]
    pub fn with_components(
        store: MemoryStore,
        embedder: Arc<dyn MemoryEmbedder>,
        consolidator: Arc<dyn MemoryConsolidator>,
    ) -> Self {
        Self {
            store,
            embedder,
            consolidator,
        }
    }

    pub fn store(&self) -> &MemoryStore {
        &self.store
    }

    #[cfg(test)]
    pub fn add_message(&self, role: &str, content: &str) -> rusqlite::Result<i64> {
        self.store.add_message(role, content)
    }

    pub fn add_turn(
        &self,
        user_content: &str,
        character_content: &str,
    ) -> rusqlite::Result<i64> {
        self.store.add_turn(user_content, character_content)
    }

    pub fn build_context(
        &self,
        query: &str,
        config: &MemoryConfig,
    ) -> rusqlite::Result<MemoryContext> {
        let mut recent_messages = self
            .store
            .get_recent_messages(config.working_max_messages)?;
        enforce_character_budget(&mut recent_messages, config.working_max_chars);

        let query_vector = self.embedder.embed(query);
        let scored_facts = self.store.retrieve_facts(
            &query_vector,
            config.retrieve_facts,
            config.recency_halflife_days,
        )?;
        let scored_episodes = self.store.retrieve_episodes(
            &query_vector,
            config.retrieve_episodes,
            config.recency_halflife_days,
        )?;

        let fact_ids: Vec<i64> = scored_facts.iter().map(|item| item.fact.id).collect();
        self.store.touch_facts(&fact_ids)?;

        let facts = scored_facts
            .into_iter()
            .filter(|item| item.score > 0.01)
            .map(|item| item.fact)
            .collect::<Vec<_>>();
        let episodes = scored_episodes
            .into_iter()
            .filter(|item| item.score > 0.01)
            .map(|item| item.episode)
            .collect::<Vec<_>>();
        let long_term_text = render_long_term_memory(&facts, &episodes);

        Ok(MemoryContext {
            recent_messages,
            facts,
            long_term_text,
        })
    }

    pub fn process_after_turn(
        &self,
        user_message: &str,
        user_message_id: i64,
        config: &MemoryConfig,
    ) -> rusqlite::Result<()> {
        for (fact, importance, source_id) in
            extract_fact_candidates(user_message, user_message_id)
        {
            let vector = self.embedder.embed(&fact);
            self.store.add_or_reinforce_fact(
                &fact,
                importance,
                &vector,
                source_id,
                config.dedupe_similarity,
            )?;
        }

        if self.store.count_unconsolidated_messages()? >= config.consolidate_after {
            self.consolidate_once(config)?;
            self.merge_chapters(config)?;
        }
        self.store.enforce_retention(
            config.max_messages,
            config.max_facts,
            config.max_episodes,
            false,
            false,
        )?;
        Ok(())
    }

    pub fn recent_messages(&self, limit: usize) -> rusqlite::Result<Vec<MemoryMessage>> {
        self.store.get_recent_messages(limit)
    }

    pub fn message_history(
        &self,
        limit: usize,
        before_id: Option<i64>,
    ) -> rusqlite::Result<Vec<MemoryMessage>> {
        self.store.get_message_history(limit, before_id)
    }

    pub fn facts(&self, limit: usize) -> rusqlite::Result<Vec<SemanticFact>> {
        self.store.get_facts(limit)
    }

    pub fn episodes(&self, limit: usize) -> rusqlite::Result<Vec<EpisodeSummary>> {
        self.store.get_episodes(limit)
    }

    pub fn stats(&self) -> rusqlite::Result<MemoryStats> {
        self.store.stats()
    }

    pub fn maintain(
        &self,
        config: &MemoryConfig,
        compact: bool,
        checkpoint: bool,
    ) -> rusqlite::Result<MemoryMaintenanceReport> {
        self.store.enforce_retention(
            config.max_messages,
            config.max_facts,
            config.max_episodes,
            compact,
            checkpoint,
        )
    }

    pub fn embedder_id(&self) -> &str {
        self.embedder.id()
    }

    fn consolidate_once(&self, config: &MemoryConfig) -> rusqlite::Result<()> {
        let messages = self.store.get_unconsolidated_messages(
            config.consolidate_after,
            config.consolidate_keep,
        )?;
        if messages.is_empty() {
            return Ok(());
        }
        let output = self.consolidator.consolidate(&messages);
        if output.summary.trim().is_empty() {
            return Ok(());
        }

        let summary_vector = self.embedder.embed(&output.summary);
        let start_id = messages.first().map(|message| message.id);
        let end_id = messages.last().map(|message| message.id);
        self.store
            .add_episode(0, &output.summary, &summary_vector, start_id, end_id)?;

        for (fact, importance, source_id) in output.facts {
            let vector = self.embedder.embed(&fact);
            self.store.add_or_reinforce_fact(
                &fact,
                importance,
                &vector,
                source_id,
                config.dedupe_similarity,
            )?;
        }
        let ids: Vec<i64> = messages.iter().map(|message| message.id).collect();
        self.store.mark_messages_consolidated(&ids)
    }

    fn merge_chapters(&self, config: &MemoryConfig) -> rusqlite::Result<()> {
        for level in 0..8 {
            let episodes = self
                .store
                .get_unmerged_episodes(level, config.chapter_merge_at)?;
            if episodes.len() < config.chapter_merge_at {
                continue;
            }
            let summary = condense(
                &format!(
                    "长期章节摘要：{}",
                    episodes
                        .iter()
                        .map(|episode| episode.summary.as_str())
                        .collect::<Vec<_>>()
                        .join("；")
                ),
                4000,
            );
            let vector = self.embedder.embed(&summary);
            let start_id = episodes.first().and_then(|episode| episode.start_message_id);
            let end_id = episodes.last().and_then(|episode| episode.end_message_id);
            self.store
                .add_episode(level + 1, &summary, &vector, start_id, end_id)?;
            let ids: Vec<i64> = episodes.iter().map(|episode| episode.id).collect();
            self.store.mark_episodes_merged(&ids)?;
        }
        Ok(())
    }
}

fn render_long_term_memory(
    facts: &[SemanticFact],
    episodes: &[EpisodeSummary],
) -> String {
    let mut output = String::new();
    if !facts.is_empty() {
        output.push_str("[Relevant facts]\n");
        for fact in facts {
            output.push_str("- ");
            output.push_str(&fact.fact);
            output.push('\n');
        }
    }
    if !episodes.is_empty() {
        output.push_str("[Relevant past episodes]\n");
        for episode in episodes {
            output.push_str("- ");
            output.push_str(&episode.summary);
            output.push('\n');
        }
    }
    output
}

fn enforce_character_budget(messages: &mut Vec<MemoryMessage>, max_chars: usize) {
    let mut current: usize = messages.iter().map(|message| message.content.chars().count()).sum();
    while messages.len() > 1 && current > max_chars {
        let removed = messages.remove(0);
        current = current.saturating_sub(removed.content.chars().count());
    }
    if let Some(message) = messages.first_mut() {
        if message.content.chars().count() > max_chars {
            message.content = message
                .content
                .chars()
                .rev()
                .take(max_chars)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
        }
    }
}

fn extract_fact_candidates(
    message: &str,
    source_message_id: i64,
) -> Vec<(String, f32, Option<i64>)> {
    let markers = [
        "我叫",
        "我是",
        "我的",
        "我喜欢",
        "我不喜欢",
        "我讨厌",
        "我住",
        "我在",
        "我希望",
        "记住",
        "记得",
        "my name",
        "i am",
        "i'm",
        "i like",
        "i dislike",
        "i live",
        "remember",
        "my ",
    ];
    split_sentences(message)
        .into_iter()
        .filter_map(|sentence| {
            let trimmed = sentence.trim();
            let lower = trimmed.to_lowercase();
            if trimmed.chars().count() < 3
                || trimmed.chars().count() > 320
                || !markers.iter().any(|marker| lower.contains(marker))
            {
                return None;
            }
            let importance = if lower.contains("记住") || lower.contains("remember") {
                8.0
            } else {
                6.0
            };
            Some((trimmed.to_string(), importance, Some(source_message_id)))
        })
        .collect()
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        current.push(character);
        if matches!(character, '。' | '！' | '？' | '!' | '?' | '\n') {
            if !current.trim().is_empty() {
                sentences.push(current.trim().to_string());
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }
    sentences
}

fn memory_tokens(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let characters: Vec<char> = lower
        .chars()
        .filter(|character| !character.is_whitespace() && !character.is_ascii_punctuation())
        .collect();
    let mut tokens = Vec::new();
    for character in &characters {
        if !character.is_ascii() {
            tokens.push(character.to_string());
        }
    }
    for window in characters.windows(2) {
        tokens.push(window.iter().collect());
    }
    tokens.extend(
        lower
            .split(|character: char| !character.is_alphanumeric())
            .filter(|token| token.len() >= 2)
            .map(ToString::to_string),
    );
    tokens
}

fn normalize_vector(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector {
            *value /= norm;
        }
    }
}

fn condense(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        let mut value = normalized.chars().take(max_chars).collect::<String>();
        value.push('…');
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("everchara-service-{label}-{unique}"))
            .join("memory.db")
    }

    #[test]
    fn hash_embedder_recognizes_related_chinese_text() {
        let embedder = HashEmbedder::default();
        let related = db::cosine_similarity(
            &embedder.embed("我喜欢草莓蛋糕"),
            &embedder.embed("草莓蛋糕是我喜欢的甜点"),
        );
        let unrelated = db::cosine_similarity(
            &embedder.embed("我喜欢草莓蛋糕"),
            &embedder.embed("今天服务器重启了"),
        );
        assert!(related > unrelated);
    }

    #[test]
    fn service_extracts_and_recalls_facts() {
        let path = temp_db("recall");
        let store = MemoryStore::new(&path, "Test").unwrap();
        let service = MemoryService::with_components(
            store,
            Arc::new(HashEmbedder::default()),
            Arc::new(HeuristicConsolidator),
        );
        let config = MemoryConfig::default();
        let user_id = service.add_message("user", "请记住，我喜欢草莓蛋糕。").unwrap();
        service
            .add_message("character", "好的，我会记住。")
            .unwrap();
        service
            .process_after_turn("请记住，我喜欢草莓蛋糕。", user_id, &config)
            .unwrap();

        let context = service.build_context("我喜欢什么甜点？", &config).unwrap();
        assert!(context
            .facts
            .iter()
            .any(|fact| fact.fact.contains("草莓蛋糕")));
        drop(service);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn consolidation_creates_episode_and_keeps_recent_messages() {
        let path = temp_db("consolidate");
        let store = MemoryStore::new(&path, "Test").unwrap();
        let service = MemoryService::with_components(
            store,
            Arc::new(HashEmbedder::default()),
            Arc::new(HeuristicConsolidator),
        );
        let mut config = MemoryConfig::default();
        config.consolidate_after = 6;
        config.consolidate_keep = 2;
        config.chapter_merge_at = 3;

        let mut last_user_id = 0;
        for index in 0..3 {
            last_user_id = service
                .add_message("user", &format!("我的测试编号是{index}。"))
                .unwrap();
            service.add_message("character", "收到。").unwrap();
        }
        service
            .process_after_turn("我的测试编号是2。", last_user_id, &config)
            .unwrap();
        assert_eq!(service.episodes(10).unwrap().len(), 1);
        assert_eq!(service.store().count_unconsolidated_messages().unwrap(), 2);
        drop(service);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn higher_episode_levels_merge_without_new_lower_level_episodes() {
        let path = temp_db("higher-level");
        let store = MemoryStore::new(&path, "Test").unwrap();
        let service = MemoryService::with_components(
            store,
            Arc::new(HashEmbedder::default()),
            Arc::new(HeuristicConsolidator),
        );
        for index in 0..3 {
            service
                .store()
                .add_episode(
                    1,
                    &format!("章节片段 {index}"),
                    &[1.0, 0.0],
                    Some(index),
                    Some(index),
                )
                .unwrap();
        }
        let mut config = MemoryConfig::default();
        config.chapter_merge_at = 3;
        service.merge_chapters(&config).unwrap();
        assert!(service
            .episodes(10)
            .unwrap()
            .iter()
            .any(|episode| episode.level == 2));
        drop(service);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
