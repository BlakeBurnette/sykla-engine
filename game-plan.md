# Stop Hiring for Roles an Agent Can Do

## Game Plan

---

## Phase 0: Define What You're Selling (Week 1)

Pick 3 roles that small companies hire for that an agent can replace on day one:

### Role 1: Receptionist / Intake
- **Trigger:** email or form submission comes in
- **Agent does:** reads it, categorizes, routes to right person, sends acknowledgment, logs in CRM
- **Replaces:** $35-45k/yr hire

### Role 2: Appointment / Schedule Coordinator
- **Trigger:** client wants to book, reschedule, cancel
- **Agent does:** checks availability, proposes times, confirms, sends calendar invites, follows up
- **Replaces:** $35-50k/yr hire

### Role 3: Invoice / Payment Chaser
- **Trigger:** invoice overdue by X days
- **Agent does:** sends reminder sequence, escalates tone, logs communication, flags for human at final stage
- **Replaces:** $40-55k/yr hire (or 10hrs/week of owner's time)

These three share a pattern: **trigger → read context → take action → log it.** Same engine, different workflows.

---

## Phase 1: Build the MVP (Weeks 2-8)

### Week 2-3: Core Engine
- Vertex AI integration (Llama 70B)
- Workflow definition format (plain English → structured steps)
- Execution loop: trigger → LLM reasons → acts → logs

### Week 4-5: First 3 Connectors
- Gmail (read, draft, send, label)
- Google Calendar (read, create, modify)
- Stripe or QuickBooks (read invoices, send reminders)

### Week 6-7: Approval + Dashboard
- "Here's what I'm about to do" review screen
- Activity log — everything the agent did
- Confidence threshold — auto-execute above X%

### Week 8: Polish + Deploy
- Auth flows (OAuth for each tool)
- Onboarding — connect tools, define first workflow
- Hosted on Cloud Run + Vertex AI

**MVP cost to build: ~$330/month in infra. Your time.**

---

## Phase 2: Get 10 Paying Customers (Weeks 9-14)

### Target
Owners/operators of 2-30 person service businesses:
- Property management companies (PayHive overlap)
- Marketing agencies
- Accounting firms
- Legal practices
- Trades companies with an office manager

### Pricing
- **$500/month** per workflow
- **$1,500/month** unlimited workflows
- **Pitch:** "Your next hire costs $4k/month and needs training. This costs $1,500 and works on day one."

### Channel
- Your existing network first
- LinkedIn content: "I replaced my [role] with an AI agent. Here's exactly what it does."
- Local business owner groups
- Property management associations (PayHive network)

### Goal
10 customers x $1,500 = **$15,000 MRR**

---

## Phase 3: Expand the Role Library (Months 4-6)

Every customer teaches you a new role:

### Roles you ship with (Phase 1):
- Receptionist / Intake
- Schedule Coordinator
- Invoice Chaser

### Roles customers ask for (Phase 3):
- Onboarding Coordinator
- Social Media Poster
- Report Generator
- Lead Follow-up
- Vendor Coordinator
- Review Requester (ask happy clients for Google reviews)

Each role is just a workflow template. Customer can use it as-is or customize. You build a library.

**This becomes a marketplace eventually** — customers share workflow templates, you curate the best ones.

---

## Phase 4: Add Connectors Based on Demand (Months 4-8)

| Priority | Timeline | Tools |
|----------|----------|-------|
| Tier 1 | Launch | Gmail, Google Calendar, Stripe/QBO |
| Tier 2 | Month 3 | Slack, HubSpot/Salesforce, Asana/Jira |
| Tier 3 | Month 5 | QuickBooks, Xero, Gusto, Square |
| Tier 4 | Month 7 | Industry-specific (AppFolio, Buildium for property mgmt, Clio for legal, etc.) |

---

## Phase 5: The Security Moat (Month 6+)

Layer in the decentralized / secure angle:

> "Unlike Zapier + ChatGPT, your data never touches OpenAI. Our agent runs on open models we control. Your workflows, your data, your infrastructure if you want it."

This is the enterprise upsell. Same product, but deployed in their environment. **$5-10k/month.**

---

## The Numbers

| Timeline | Customers | MRR |
|----------|-----------|-----|
| Month 3 | 10 | $15,000 |
| Month 6 | 50 | $75,000 |
| Month 12 | 200 | $300,000 |

**Cost to serve 200 customers on Vertex AI: ~$60-200/month**
**Margin: 99%+**

### Hiring Plan
- **Month 4-5:** First hire — someone to build connectors
- **Month 6-7:** Second hire — someone to handle sales/onboarding

---

## Unit Economics

### Cost Per Customer
| Usage Level | Vertex AI Cost | Price | Margin |
|-------------|---------------|-------|--------|
| Light (500 queries/mo) | $0.10 | $1,500/mo | 99.9% |
| Normal (1,500 queries/mo) | $0.30 | $1,500/mo | 99.9% |
| Heavy (5,000 queries/mo) | $1.00 | $1,500/mo | 99.9% |
| Power (20,000 queries/mo) | $4.00 | $1,500/mo | 99.7% |

### Break-even: 1 paying customer.

---

## Infrastructure

```
Vertex AI (Llama 70B)
       ↑
Cloud Run (API gateway + schema enforcement + execution engine)
       ↑
Cloudflare (rate limiting, DDoS, caching)
       ↑
Customer
```

No VMs to manage. No GPUs to provision. Serverless everything.

---

## What to Do Monday

1. Set up Vertex AI with Llama 70B
2. Build the execution engine — trigger → LLM → action → log
3. Wire up Gmail as the first connector
4. Define the receptionist workflow as the first template
5. Use it yourself for 2 weeks

**If you can't replace one of your own repetitive tasks with it, nobody else will pay for it.**

---

## The Four Problems This Solves

1. **AI is insecure** — runs on open models you control, customer data never touches OpenAI/Anthropic
2. **AI doesn't behave like software** — typed inputs/outputs, real error codes, deterministic behavior
3. **AI requires a full redesign** — customer defines workflows in plain English, no engineering required
4. **AI can be turned off by whoever's in power** — open models, your infrastructure, can't be killed by an executive order

---

## Decentralized AI (Longer Term)

The decentralized training network becomes relevant when:
- You need to improve the model beyond base Llama
- You have customer usage data showing where the model fails
- You want to offer "truly independent" as an enterprise feature

### Token Economics (Option A: Credits first)
- Contributors earn compute credits for GPU time donated to training
- Credits redeemable for inference / service access
- Non-transferable initially (no legal risk)
- Migrate to on-chain token (Option B) when scale justifies it

### Cost to Self-Host (if needed)
| Model | On-demand (70 hrs/mo) | Spot (70 hrs/mo) |
|-------|----------------------|-------------------|
| 70B (1x A100) | $350 | $105-140 |
| 405B (8x A100) | $2,800 | $840-1,120 |
