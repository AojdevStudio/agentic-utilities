# YouTube Content Type Detection System

**Purpose:** Two-dimensional classification system for routing YouTube video analysis to the correct output location and analysis workflow.

> **CRITICAL CONFIDENCE RULE:** If detection confidence is below 60%, you MUST present the top 2-3 alternatives to the user via AskUserQuestion and let them choose. Do NOT auto-classify with low confidence. Ties in top scores also require user confirmation. This is enforced by BLOCKING GATE 2 in SKILL.md.

---

## Two-Dimensional Classification

YouTube content is classified along **two independent axes**:

1. **Category Axis** - Determines WHERE the output is saved
2. **Format Axis** - Determines HOW the content is analyzed

**Example:**
- Video: "React 19 Tutorial - Build a Full Stack App"
- **Category:** `technology` (used in YAML frontmatter and as a suggested subdirectory if you organize hierarchically)
- **Format:** `tutorial` → uses `workflows/tutorial-workflow.md` for analysis

> Note: in this plugin, the actual output directory is resolved from `.claude/youtube-analyzer.local.md` (set on first run). Category and format do NOT change the destination directory — they only affect YAML frontmatter and the workflow chosen.

---

## Axis 1: Category (Topic Classification)

**Category labels the topical area of the video. It's recorded in the analysis YAML frontmatter and can be used as a suggested subdirectory if the user organizes their output directory hierarchically.**

| Category | Suggested Subdirectory | Description |
|----------|------------------------|-------------|
| `business` | `business/` | Entrepreneurship, management, strategy, leadership, startup advice |
| `education` | `education/` | Learning resources, courses (non-technical), study techniques, academic content |
| `entertainment` | `entertainment/` | Gaming, movies, TV, pop culture, comedy, media analysis |
| `finance` | `finance/` | Investing, trading, portfolio management, dividends, market analysis, personal finance |
| `general` | `general/` | Mixed topics, uncategorized content, multi-domain discussions |
| `health` | `health/` | Wellness, fitness, nutrition, mental health, medical information |
| `politics` | `politics/` | Policy analysis, governance, elections, international relations, political commentary |
| `religion` | `religion/` | Religious content, church services, biblical teaching, spiritual exposition |
| `science` | `science/` | Research, discoveries, scientific method, experiments, academic science |
| `social-media` | `social-media/` | Social media strategy, content creation, influencer marketing, platform analysis |
| `technology` | `technology/` | AI/ML, programming, software architecture, DevOps, tech news, frameworks |

---

## Axis 2: Format (Analysis Workflow)

**Format determines which specialized workflow analyzes the content.**

| Format | Workflow | Detection Signals | Characteristics |
|--------|----------|-------------------|-----------------|
| `tutorial` | `workflows/tutorial-workflow.md` | "tutorial", "how to", "build", "code along", step-by-step instruction, code in description, timestamps for sections | Instructional, actionable, teaches a skill or process |
| `course` | `workflows/tutorial-workflow.md` | "full course", "complete course", "bootcamp", multi-hour duration, structured curriculum, chapter markers | Comprehensive educational program, often 2+ hours |
| `finance` | `workflows/finance-workflow.md` | "investing", "portfolio", "dividend", "stock market", "trading", finance-specific channels | Investment analysis, market commentary, financial strategies |
| `interview` | `workflows/general-workflow.md` | "interview", "conversation", "podcast", "talks with", Q&A format, two+ speakers | Conversational, question-driven, personality-focused |
| `lecture` | `workflows/general-workflow.md` | "lecture", "class", "presentation", "keynote", academic setting, single expert speaker | Formal educational presentation, often academic or conference |
| `general` | `workflows/general-workflow.md` | No strong format signals, commentary, analysis, discussion, review | Default fallback for content that doesn't fit specialized formats |

---

## Category Detection Keywords

**Each category has weighted keywords for classification. Title gets 3x weight, description 2x, tags 1x, channel 2x.**

### Business

**Keywords:**
```javascript
[
  "entrepreneur", "entrepreneurship", "startup", "business", "management",
  "leadership", "strategy", "marketing", "sales", "growth",
  "scale", "scaling", "revenue", "profit", "business model",
  "CEO", "founder", "company", "enterprise", "operations",
  "team building", "hiring", "HR", "culture", "productivity",
  "negotiation", "deal", "partnership", "acquisition", "exit"
]
```

**Known Channels:**
- Gary Vaynerchuk
- Simon Sinek
- Y Combinator
- Startup Grind
- How I Built This

---

### Education

**Keywords:**
```javascript
[
  "learn", "learning", "study", "course", "class",
  "lesson", "education", "teach", "training", "skill",
  "master", "mastery", "beginner", "intermediate", "advanced",
  "tutorial", "guide", "walkthrough", "step by step", "how to",
  "exam", "test", "certification", "degree", "academic",
  "university", "college", "school", "student", "professor"
]
```

**Known Channels:**
- Khan Academy
- Crash Course
- TED-Ed
- Coursera
- edX

---

### Entertainment

**Keywords:**
```javascript
[
  "game", "gaming", "gameplay", "playthrough", "stream",
  "movie", "film", "TV", "show", "series",
  "review", "reaction", "trailer", "cinema", "entertainment",
  "comedy", "funny", "humor", "sketch", "parody",
  "music", "song", "concert", "performance", "artist",
  "pop culture", "celebrity", "viral", "meme", "trending"
]
```

**Known Channels:**
- PewDiePie
- IGN
- GameSpot
- Red Letter Media
- Dunkey

---

### Finance

**Keywords:**
```javascript
[
  "invest", "investing", "investment", "portfolio", "stock",
  "dividend", "dividends", "passive income", "FIRE", "financial independence",
  "margin", "options", "trading", "market", "bull market",
  "bear market", "recession", "inflation", "fed", "interest rate",
  "401k", "IRA", "retirement", "wealth", "money",
  "ETF", "mutual fund", "index fund", "bond", "real estate",
  "crypto", "bitcoin", "ethereum", "DeFi", "blockchain",
  "valuation", "earnings", "balance sheet", "cash flow", "P/E ratio"
]
```

**Known Channels:**
- Paycheck to Portfolio
- Ticker Symbol: YOU
- Margin Mindset
- Andrei Jikh
- Graham Stephan
- Meet Kevin
- Everything Money

---

### Health

**Keywords:**
```javascript
[
  "fitness", "workout", "exercise", "training", "gym",
  "nutrition", "diet", "healthy eating", "meal prep", "calories",
  "weight loss", "muscle gain", "cardio", "strength", "yoga",
  "mental health", "therapy", "anxiety", "depression", "wellness",
  "sleep", "meditation", "mindfulness", "stress", "recovery",
  "supplement", "vitamin", "protein", "health", "medical",
  "doctor", "science-based", "evidence", "study", "research"
]
```

**Known Channels:**
- Jeff Nippard
- AthleanX
- Dr. Mike Israetel
- Huberman Lab
- FoundMyFitness

---

### Politics

**Keywords:**
```javascript
[
  "politics", "political", "election", "campaign", "vote",
  "government", "congress", "senate", "house", "president",
  "policy", "legislation", "law", "bill", "regulation",
  "democrat", "republican", "liberal", "conservative", "progressive",
  "foreign policy", "diplomacy", "war", "military", "defense",
  "immigration", "healthcare", "climate", "economy", "tax",
  "constitution", "supreme court", "justice", "rights", "amendment"
]
```

**Known Channels:**
- Vox
- Vice News
- PBS NewsHour
- MSNBC
- Fox News

---

### Science

**Keywords:**
```javascript
[
  "science", "scientific", "research", "study", "experiment",
  "physics", "chemistry", "biology", "astronomy", "space",
  "quantum", "theory", "hypothesis", "evidence", "data",
  "discovery", "breakthrough", "innovation", "technology", "engineering",
  "lab", "scientist", "professor", "university", "peer review",
  "nature", "evolution", "climate", "energy", "particle",
  "DNA", "genetics", "neuroscience", "brain", "cosmos"
]
```

**Known Channels:**
- Veritasium
- Kurzgesagt
- PBS Space Time
- SmarterEveryDay
- MinutePhysics

---

### Social Media

**Keywords:**
```javascript
[
  "social media", "content creator", "influencer", "creator economy", "monetization",
  "YouTube", "TikTok", "Instagram", "Twitter", "LinkedIn",
  "viral", "algorithm", "engagement", "followers", "subscribers",
  "content strategy", "posting schedule", "thumbnail", "SEO", "analytics",
  "brand deal", "sponsorship", "affiliate", "AdSense", "revenue",
  "growth", "niche", "audience", "community", "platform"
]
```

**Known Channels:**
- Think Media
- VidIQ
- Sunny Lenarduzzi
- Roberto Blake
- Ali Abdaal (when discussing content creation)

---

### Technology

**Keywords:**
```javascript
[
  "programming", "coding", "code", "developer", "software",
  "AI", "artificial intelligence", "machine learning", "ML", "deep learning",
  "neural network", "LLM", "GPT", "ChatGPT", "Claude",
  "React", "JavaScript", "TypeScript", "Python", "Rust",
  "web development", "frontend", "backend", "full stack", "DevOps",
  "database", "SQL", "NoSQL", "API", "REST",
  "cloud", "AWS", "Azure", "GCP", "serverless",
  "Docker", "Kubernetes", "CI/CD", "microservices", "architecture",
  "framework", "library", "open source", "GitHub", "git",
  "tech news", "startup", "Silicon Valley", "venture capital", "IPO"
]
```

**Known Channels:**
- Fireship
- Theo - t3.gg
- Primeagen
- Web Dev Simplified
- Traversy Media
- Lex Fridman (tech topics)
- All-In Podcast (tech/VC topics)

---

### Religion

**Keywords:**
```javascript
[
  "sermon", "church", "pastor", "preacher", "ministry",
  "gospel", "scripture", "bible", "biblical", "God",
  "Jesus", "Christ", "Christian", "Christianity", "faith",
  "worship", "prayer", "spiritual", "salvation", "grace",
  "testimony", "disciple", "apostle", "revelation", "prophecy",
  "Sunday service", "church service", "baptism", "communion", "Holy Spirit"
]
```

**Known Channels:**
- First Baptist Church
- Elevation Church
- Life.Church
- Bethel Church
- The Bible Project

Religious content (sermons, biblical exposition) is classified under `religion` category and analyzed via `workflows/general-workflow.md`. A specialized sermon workflow may be added in the future or shipped as a separate plugin.

---

## Format Detection Keywords

**Format keywords help determine the analysis workflow (not output location).**

### Tutorial Format

**Keywords:**
```javascript
[
  "tutorial", "how to", "build", "create", "make",
  "step by step", "walkthrough", "guide", "beginner", "learn",
  "code along", "follow along", "from scratch", "complete guide", "full guide",
  "explained", "course", "lesson", "chapter", "part 1",
  "intro", "introduction", "getting started", "basics", "fundamentals"
]
```

**Signals:**
- Timestamps in description for different sections
- Code snippets in description
- "Resources" or "Links" section
- Project files/GitHub links
- Multiple parts/chapters

---

### Course Format

**Keywords:**
```javascript
[
  "full course", "complete course", "bootcamp", "masterclass", "comprehensive",
  "zero to hero", "beginner to advanced", "crash course", "deep dive", "complete guide",
  "certification", "curriculum", "syllabus", "module", "unit",
  "hours", "hour course", "full tutorial", "everything you need"
]
```

**Signals:**
- Duration > 2 hours
- Chapter markers or sections
- "Part 1", "Part 2" in series
- Structured learning path
- Certificate or completion mentioned

---

### Finance Format

**Keywords:**
```javascript
[
  "portfolio update", "dividend income", "passive income", "investing strategy", "stock analysis",
  "market update", "earnings report", "financial independence", "FIRE", "retirement",
  "covered calls", "options strategy", "margin investing", "leverage", "yield",
  "monthly dividends", "dividend growth", "buy and hold", "dollar cost averaging", "DCA"
]
```

**Signals:**
- Finance-specific channels (Paycheck to Portfolio, Ticker Symbol: YOU, etc.)
- Stock tickers in title/description
- Charts/graphs in thumbnail
- Financial data in description

---

### Interview/Podcast Format

**Keywords:**
```javascript
[
  "interview", "conversation", "talks with", "podcast", "episode",
  "Q&A", "ask me anything", "AMA", "discussion", "chat",
  "guest", "with", "featuring", "speaks to", "in conversation"
]
```

**Signals:**
- Two or more speakers
- Podcast format
- Question/answer structure
- Guest name in title
- "Ep" or "Episode" numbering

---

### Lecture Format

**Keywords:**
```javascript
[
  "lecture", "presentation", "keynote", "talk", "speech",
  "class", "seminar", "workshop", "conference", "summit",
  "explains", "breakdown", "analysis", "deep dive", "overview"
]
```

**Signals:**
- Academic or conference setting
- Single expert speaker
- Formal presentation style
- Educational institution channel
- "Professor", "Dr.", "PhD" in speaker name

---

## Confidence Scoring System

**Weighted Keyword Matching:**

```javascript
// Scoring weights
const weights = {
  title: 3,        // Title keywords count 3x
  description: 2,  // Description keywords count 2x
  tags: 1,         // Tags count 1x
  channel: 2       // Known channel match counts 2x
};

// Example calculation
Video: "How to Build a Dividend Portfolio with Margin - Full Tutorial"
Channel: "Paycheck to Portfolio"

Category Scores:
- finance: title(3) + description(2) + channel(2) = 7 points
- education: title(1) + description(1) = 2 points
- technology: 0 points

Format Scores:
- tutorial: title(3) + description(2) = 5 points
- finance: title(3) + channel(2) = 5 points
- general: 0 points

Total Possible: 8 points (max from all sources)
Finance Confidence: 7/8 = 87.5%
Tutorial/Finance (tie): Both 5 points → ask user
```

**Confidence Threshold:**

| Confidence | Action |
|------------|--------|
| **≥ 60%** | Auto-classify and proceed with detected category/format |
| **40-59%** | Present top 2 options, ask user to confirm |
| **< 40%** | Present top 3 options, strongly recommend user selection |
| **Tie** | If top scores are equal, ALWAYS ask user |

**User Confirmation Prompt (when needed):**

```
Detected content type with 52% confidence:
  Category: finance → knowledge/finance/youtube-summaries/
  Format: tutorial → TutorialWorkflow.md

Alternative options:
  1. technology + tutorial (45% confidence)
  2. business + general (38% confidence)

Proceed with finance/tutorial? [Y/n/1/2]
```

---

## Output Filename Format

**Pattern:** `{primary-topic}-{secondary-detail}-{YYYY-MM-DD}.md`

**Rules:**
1. Use kebab-case (lowercase, hyphens for spaces)
2. Extract primary topic from title (2-4 words)
3. Add secondary detail if needed for clarity (1-3 words)
4. Always include ISO date (YYYY-MM-DD)
5. Be specific but concise (5-8 words total max)

**Examples:**

| Video Title | Filename |
|-------------|----------|
| "How I Built $3K Monthly Dividends with Margin" | `building-3k-monthly-dividends-margin-2026-01-30.md` |
| "React 19 Server Components - Complete Tutorial" | `react-19-server-components-tutorial-2026-01-30.md` |
| "Palantir Q3 Earnings Analysis - Is It Overvalued?" | `palantir-q3-earnings-analysis-2026-01-30.md` |
| "Interview with Elon Musk on AI Safety" | `elon-musk-ai-safety-interview-2026-01-30.md` |
| "The Power of Faith - Sunday Morning Service" | `power-of-faith-sunday-service-2026-01-30.md` |

**Bad Examples (avoid):**

| Bad Filename | Why It's Bad | Better Version |
|--------------|--------------|----------------|
| `video-analysis-2026-01-30.md` | Too generic, no topic | `dividend-portfolio-strategy-2026-01-30.md` |
| `how-i-built-3k-monthly-dividends-with-margin-using-this-one-weird-trick-2026-01-30.md` | Too long, clickbait | `building-3k-monthly-dividends-margin-2026-01-30.md` |
| `React_Tutorial_2026.md` | Underscores, no day, too generic | `react-19-server-components-tutorial-2026-01-30.md` |
| `earnings.md` | No date, no context | `palantir-q3-earnings-analysis-2026-01-30.md` |

---

## Special Cases & Edge Cases

### Multi-Category Content

**Example:** "How AI Will Revolutionize Healthcare in 2026"

**Solution:**
- **Primary Category:** Determined by main focus (health or technology?)
- **Check description** for deeper context
- **If unclear:** Ask user which category is more relevant
- **Tie-breaker:** Use channel's primary category if known

---

### Evolving Content (Live Streams → Edits)

**Example:** Live stream becomes edited video later

**Solution:**
- Use metadata from the CURRENT video state
- If originally analyzed as live stream, re-analyze edited version as new content
- File naming includes date to allow version tracking

---

### Series/Multi-Part Content

**Example:** "React Course - Part 1 of 10"

**Solution:**
- Each part gets its own analysis file
- Filename includes part number: `react-course-part-1-fundamentals-2026-01-30.md`
- In metadata, note series info: `series: React Course, part: 1/10`

---

### Shorts/Clips (< 5 minutes)

**Solution:**
- Same classification system applies
- Format likely `general` unless clearly tutorial/finance
- Filename should still be descriptive
- Consider skipping full workflow for <2min clips (ask user)

---

### Language/Non-English Content

**Solution:**
- If transcript unavailable (non-English), rely on title/description
- Ask user if translation is needed before proceeding
- Category/format detection still works with translated metadata
- Note language in output metadata

---

## Classification Decision Tree

```
START: Receive YouTube URL
    ↓
Extract Metadata (yt-dlp)
    ↓
Check Known Channel List
    ├─ Match Found → Apply channel default category (2x weight)
    └─ No Match → Continue to keyword analysis
    ↓
Keyword Analysis (title 3x, description 2x, tags 1x)
    ↓
Calculate Category Scores
    ↓
Calculate Format Scores
    ↓
Check Confidence Levels
    ├─ Category ≥60% AND Format ≥60% → AUTO-CLASSIFY
    ├─ Category <60% OR Format <60% → PRESENT OPTIONS + ASK USER
    └─ Tie in top scores → ASK USER
    ↓
Confirm Classification with User (if needed)
    ↓
Generate Output Filename
    ↓
Route to Workflow
    ├─ tutorial/course → workflows/tutorial-workflow.md
    ├─ finance → workflows/finance-workflow.md
    └─ interview/lecture/general → workflows/general-workflow.md
    ↓
Return: {category, format, outputPath, filename, workflow}
```

---

## Usage in Analyze.md Workflow

**Step 1: Load ContentTypes.md**

```markdown
Read this file to get classification rules
```

**Step 2: Extract and Analyze**

```bash
# Get metadata
yt-dlp --skip-download --write-info-json <URL>

# Parse JSON for title, description, channel, tags, duration
```

**Step 3: Run Classification**

```javascript
// Pseudo-code for classification logic
const categoryScores = calculateCategoryScores(metadata, weights);
const formatScores = calculateFormatScores(metadata, weights);

const topCategory = getTopScore(categoryScores);
const topFormat = getTopScore(formatScores);

if (topCategory.confidence >= 0.6 && topFormat.confidence >= 0.6) {
  return { category: topCategory.name, format: topFormat.name };
} else {
  return askUserForConfirmation(topCategory, topFormat, alternatives);
}
```

**Step 4: Route to Workflow**

```markdown
Based on format, dispatch to appropriate workflow:
- workflows/tutorial-workflow.md (for tutorial/course)
- workflows/finance-workflow.md (for finance)
- workflows/general-workflow.md (for interview/lecture/general)
```

---

## Maintenance & Updates

**Adding New Categories:**
1. Add category to Category Mapping table
2. Define output path in filesystem
3. Add keyword array (20+ keywords)
4. Update decision tree logic
5. Test with 5+ example videos

**Adding New Formats:**
1. Add format to Format Mapping table
2. Create corresponding workflow file (if needed)
3. Add keyword array (15+ keywords)
4. Update routing logic in Analyze.md
5. Test with 5+ example videos

**Tuning Detection:**
- Track false positives/negatives in a log
- Adjust weights if category consistently misclassifies
- Add channel overrides for known creators
- Refine keyword lists based on user corrections

---

**End of ContentTypes.md**

This document is the authoritative reference for all content type detection in YouTubeAnalyzer. When in doubt, consult this file. When classification is ambiguous, ask the user.
