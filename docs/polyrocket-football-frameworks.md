# 业界权威足球菠菜分析框架 (Football Betting Analysis Frameworks)

调研时间: 2026-06-22
目标: 用于 polyrocket 项目,作为 football.v1.0 prompt 的知识库

---

## 1. Dixon-Coles Model (1997) — 学术金标准

**来源**: Dixon, M.J. & Coles, S.G. (1997). "Modelling Association Football Scores and Inefficiencies in the Football Betting Market". Journal of the Royal Statistical Society, Series C (Applied Statistics), 46(2), 265-280.

**核心思想**:
- 在 Maher (1982) 双 Poisson 基础上,加入 low-score 调整 (0-0, 0-1, 1-0, 1-1 之间的概率调整, 即 ρ 参数)
- 球队独立的 attack/defense 参数 (攻击强度 + 防守强度)
- 时间动态 (team strength varies over time)
- 跨联赛杯赛兼容 (cross-league cup matches)
- 用最大似然估计 (MLE) 拟合
- 对 1992-1995 英超数据 + 1995-1996 赔率回测,**positive return**

**关键公式**:
```
P(home=i, away=j) = τ(i,j) · λ_home^i · e^(-λ_home) / i! · λ_away^j · e^(-λ_away) / j!
τ(i,j) = 低分修正 (rho 参数调整)
λ_home = attack_home · defense_away · home_advantage_factor
λ_away = attack_away · defense_home
```

**业界地位**: 全球几乎所有商业足球博彩模型的数学基础。

---

## 2. Elo Rating (Arpad Elo) — 通用球队评级

**来源**: Elo, A. (1978). "The Rating of Chessplayers, Past and Present". 
**足球应用**: FiveThirtyEight (Nate Silver 团队), clubelo.com, Football Manager

**核心思想**:
- 每支球队一个 rating 数值 (国际象棋 1500-2800, 足球 ~1300-2000)
- 赛前计算期望胜率 (logistic): E_A = 1 / (1 + 10^((R_B - R_A)/400))
- 赛后更新: R_A_new = R_A + K · (S_A - E_A)
  - K = 16-32 (master 级 16, 普通 32)
  - S_A = 1 (胜) / 0.5 (平) / 0 (负)
- 主场优势 ~ +100 分

**FiveThirtyEight 改进**:
- 比赛重要性加权 (世界杯 > 联赛 > 友谊赛)
- 联赛降级保护 (刚升级球队不重置)
- 加权回归到 mean (避免 rating drift)

**业界地位**: Soccer Power Index (SPI) 的核心组件。

---

## 3. xG (Expected Goals) — 现代足球分析的核心指标

**来源**: Sam Green (Opta) 2012 引入,广泛采用 by Sky Sports, BBC, NBC, Sky Germany, StatsBomb。

**核心思想**:
- 每次射门基于历史相似射门数据计算"进球概率"
- xG ∈ [0, 1], 0=必不进, 1=必进
- 一次射门的 xG 反映"质量",而非结果 (运气)

**特征输入** (StatsBomb 公开数据):
1. **射门位置** (球门距离 + 角度)
2. **射门身体部位** (左/右脚/头)
3. **传球类型 / 进攻模式** (定位球 / 反击 / 阵地战)
4. **守门员位置** (freeze frame)
5. **封堵球员数量 + 位置** (free frame)
6. **5 码内干扰球员**
7. **射门技术** (凌空 / 抽射 / 推射)
8. **是否第一次触球**
9. **上下半场**

**点球固定 xG = 0.75**

**派生指标**:
- **xGA** (Expected Assists): 传球成为助攻的概率
- **xPTS** (Expected Points): 球队预期获得积分
- **xG+** (Pipping-Gamón 2025): 把射门概率建模为联合概率 (射门发生 × 射门得分)
- **xGChain**: 一次射门之前所有参与的连续动作累计 xG

**业界地位**: 当前最普及的高级指标,转会市场、教练战术都依赖 xG。

---

## 4. Pinnacle Closing Line Value (CLV) — 业界"edge"标准定义

**来源**: Pinnacle Sports (业内 sharpest bookmaker) 的公开文章 + 学术论文 (Constantinides & Swaminathan, Forbes 等)

**核心思想**:
- **Closing Line** = 比赛开始前最后的市场赔率 (隐含概率)
- Pinnacle 的 closing line 被认为是 **效率最高的预测市场** (lowest vig, sharpest action)
- **CLV** = 你自己的概率 vs closing line implied prob 之间的差
- 如果你长期 +CLV, 就有 +EV (市场效率假说成立)
- **CLV 是预测者唯一长期可持续的盈利指标** (单场胜负率不可靠, 样本太小时)

**Polymarket 类比**:
- Polymarket 是 prediction market, 没有"传统意义的 closing line"
- 但 **CLOB order book 的最后成交价 / mid-price** 可以视为类似 closing line
- 我们应该把 LLM 估计的 P(YES) 与 Polymarket mid-price 比较, 算 "edge"

**业界地位**: 所有严肃 bettor 的核心 KPI。

---

## 5. Kelly Criterion (1956) — stake sizing

**来源**: Kelly, J.L. (1956). "A New Interpretation of Information Rate".

**公式**: f* = (bp - q) / b
- b = 净赔率 (decimal odds - 1)
- p = 你估计的胜率
- q = 1 - p
- f* = 最优下注比例 (bankroll 的 %)

**业界实践**:
- Half-Kelly 或 Quarter-Kelly (避免方差过大)
- 用于计算 "edge" 之后的具体仓位

---

## 6. Asian Handicap (亚盘) — 足球专属盘口结构

**特点**:
- 消除平局 (只可能 win/lose)
- 半球 / 一球 / 球半: 0.25 / 0.5 / 0.75 / 1 / 1.25 ...
- Polymarket 类似的 spread 盘: "France -2.5" 等

**与 Poisson 模型映射**:
- Asian Handicap 直接对应 "净胜球" 的累积概率分布
- Dixon-Coles 的 score matrix → 累加 → Asian Handicap prob
- 例如 AH -1 = P(净胜 ≥ 1) - P(净胜 ≤ -2)

**业界地位**: 亚洲市场主盘, 流动性最强。

---

## 7. Goal Expectancy (进球率预测法) — Jackson & Moschewski 1990

**核心**:
- 4 规则基于双方平均进球率差:
  - 差 > X: 高进球率队胜
  - 0 < 差 < X: 主场队平均进球率高则主场胜
  - 等等
- 英超 + 意甲准确率最高 (1990 paper)

**业界地位**: 早期模型, 现在已被 Dixon-Coles 取代。

---

## 8. GAP Ratings (Generalised Attacking Performance) — Wheatcroft 2019

**来源**: "A profitable model for predicting the over/under market in football", ScienceDirect.

**核心**:
- 用于 Over/Under 2.5 大小球市场
- 输入: shots + shots on target (而非 goals)
- 跨 10 个欧洲联赛 12 年回测: 0.8% ROI / bet

**意义**: 把 shot-based metric 应用到大小球, 替代 goals-based model。

---

## 9. 综合推荐: 哪些 framework 用于 polyrocket football.v1.0

| Framework | 用于哪个市场 | 优先级 |
|---|---|---|
| **xG** | 所有 football 市场 (form 评估 + 临场调整) | ⭐⭐⭐ 必选 |
| **Dixon-Coles** | 1X2 (胜平负) + O/U 大小球 + Asian Handicap | ⭐⭐⭐ 必选 (黄金标准) |
| **Elo / SPI** | 1X2 整体概率 (球队相对实力) | ⭐⭐ 重要 |
| **CLV** | Polymarket edge 计算 (LLM prob vs Polymarket mid) | ⭐⭐⭐ 必选 (输出维度) |
| **Kelly** | 推荐仓位 (bankroll %) | ⭐ 可选 (后续 round) |
| **Asian Handicap mapping** | Spread 盘 (Polymarket "France -2.5") | ⭐⭐ 重要 |
| **GAP** | 大小球 (替代 goals 的 shot-based 评估) | ⭐ 备用 |
| **Goal Expectancy** | 历史方法, 仅作 backup | 可选 |

---

## 10. polyrocket football.v1.0 prompt 的核心 schema

基于以上 framework, 输出 JSON 应该是:

```json
{
  "probability": 0.0-1.0,              // 1X2 中 "home win" 概率 (主队胜出)
  "side": "YES" | "NO" | "skip",       // 跟 Polymarket YES/NO 对齐
  "confidence": 0.0-1.0,               // 模型自身置信度
  "reasoning": "string ≤ 800 chars",   // 综合 Dixon-Coles + Elo + xG + 上下文

  "framework_breakdown": {             // NEW football-specific fields
    "elo_diff": -400..+400,            // Elo rating 差 (主场 - 客场)
    "implied_win_pct_elo": 0.0-1.0,    // Elo 期望胜率
    "dixon_coles_home_goals_lambda": 0.0+,   // Poisson λ_home
    "dixon_coles_away_goals_lambda": 0.0+,   // Poisson λ_away
    "xg_last_5_diff": -2.0..+2.0,      // 双方近 5 场 xG 差
    "asian_handicap_edge": -0.5..+0.5, // vs Polymarket spread 的 edge
    "polymarket_implied_prob": 0.0-1.0,
    "clv_edge": -0.5..+0.5             // probability - polymarket_implied_prob
  },

  "key_factors": ["...", "..."] (≤ 5 strings)
}
```

这个 schema 同时支持:
- 1X2 (胜平负) 市场 (Polymarket "Argentina win?")
- Spread (让球) 市场 (Polymarket "France -2.5")
- O/U (大小球) 市场 (Polymarket "Argentina vs Austria O/U 2.5")
- Draw 市场 (Polymarket "Argentina vs Austria draw?")
