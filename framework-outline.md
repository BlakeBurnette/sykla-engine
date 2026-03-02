# Agent Framework — Technical Outline

## What It Is

A framework that lets engineers automate multi-tool workflows by describing them in plain English. The framework handles model routing, tool execution, retries, structured I/O, security, and observability. Business owners get the same engine through a no-code dashboard.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      USER LAYER                          │
│                                                          │
│   SDK (Python/TS)          Dashboard (Web UI)            │
│   Engineers write code     Business owners click & type  │
└────────────┬─────────────────────────┬──────────────────┘
             │                         │
┌────────────▼─────────────────────────▼──────────────────┐
│                     API SERVER                           │
│                                                          │
│   REST + WebSocket                                       │
│   Auth (API keys, OAuth)                                 │
│   Rate limiting                                          │
│   Request routing                                        │
└────────────┬────────────────────────────────────────────┘
             │
┌────────────▼────────────────────────────────────────────┐
│                   CORE ENGINE                            │
│                                                          │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│   │  Workflow     │  │   Model      │  │  Security    │ │
│   │  Engine       │  │   Router     │  │  Layer       │ │
│   │              │  │              │  │              │ │
│   │  Parse       │  │  Vertex AI   │  │  Encryption  │ │
│   │  Schedule    │  │  Ollama      │  │  PII redact  │ │
│   │  Execute     │  │  OpenAI      │  │  Audit log   │ │
│   │  Retry       │  │  Any OAIAPI  │  │  Permissions │ │
│   │  Approve     │  │  Fallover    │  │              │ │
│   │  Log         │  │  A/B test    │  │              │ │
│   └──────┬───────┘  └──────────────┘  └──────────────┘ │
│          │                                               │
│   ┌──────▼───────┐  ┌──────────────┐  ┌──────────────┐ │
│   │  Tool        │  │  Schema      │  │  State       │ │
│   │  Executor    │  │  Enforcer    │  │  Manager     │ │
│   │              │  │              │  │              │ │
│   │  Call APIs   │  │  Validate    │  │  Workflow    │ │
│   │  Handle auth │  │  in/out      │  │  memory      │ │
│   │  Parse       │  │  Retry on    │  │  Timers      │ │
│   │  response    │  │  bad output  │  │  Queues      │ │
│   │  Error codes │  │  Type coerce │  │  Checkpoints │ │
│   └──────────────┘  └──────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────┘
             │
┌────────────▼────────────────────────────────────────────┐
│                   CONNECTOR LAYER                        │
│                                                          │
│   Each connector: read, write, watch (triggers)          │
│                                                          │
│   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐          │
│   │ Gmail  │ │ Slack  │ │ Jira   │ │ Stripe │          │
│   └────────┘ └────────┘ └────────┘ └────────┘          │
│   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐          │
│   │ GitHub │ │Calendar│ │HubSpot │ │  QBO   │          │
│   └────────┘ └────────┘ └────────┘ └────────┘          │
│   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐          │
│   │Retell  │ │Twilio  │ │Datadog │ │  AWS   │          │
│   └────────┘ └────────┘ └────────┘ └────────┘          │
│   ┌──────────────────────────────────────────┐          │
│   │ Generic REST / GraphQL / Webhook          │          │
│   │ (any tool with an API)                    │          │
│   └──────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────┘
```

---

## Core Components — What Each Does

### 1. Workflow Engine

The brain. Takes a plain English workflow description, breaks it into executable steps, runs them.

```
Input:  "When a Jira ticket is created, read it,
         create a Retell agent, generate Terraform,
         open a PR, run tests, update the ticket"

Engine does:
  1. Parse → identify trigger (Jira webhook)
  2. Parse → identify steps and dependencies
  3. Parse → identify tools needed (Jira, Retell, GitHub)
  4. Register trigger with connector
  5. When triggered:
     a. Execute steps sequentially
     b. Pass context between steps (output of step 1 → input of step 2)
     c. Handle branches (if tests pass → do X, else → do Y)
     d. Handle failures (retry, skip, escalate)
     e. Log everything
```

Key features:
- **Plain English parsing** — LLM converts description to execution plan
- **Step dependencies** — output of one step feeds into next
- **Branching** — if/else based on LLM judgment or explicit conditions
- **Timers** — "follow up in 24 hours" creates a scheduled re-execution
- **Human-in-the-loop** — pause and wait for approval at any step
- **Checkpointing** — if step 4 fails, resume from step 4 not step 1

### 2. Model Router

Handles all model communication. Engineer never thinks about this.

- **Primary + fallback** — Vertex down? Auto-switch to Ollama or OpenAI
- **Model selection per step** — simple classification → 8B, complex reasoning → 70B
- **Streaming** — real-time output for long generations
- **Caching** — identical inputs return cached output instantly
- **Cost tracking** — per workflow, per step, per customer
- **Rate limit handling** — queue and retry, never fail on 429

### 3. Schema Enforcer

The thing that makes AI output actually usable in code.

```python
# Engineer defines expected output
class Invoice(Schema):
    vendor: str
    amount: float
    due_date: str
    line_items: list[LineItem]

# Framework guarantees this shape
# If model returns garbage:
#   1. Retry with stricter prompt (up to 3 times)
#   2. Attempt to coerce (string "45.00" → float 45.0)
#   3. If still invalid, return typed error, not an exception
```

### 4. Tool Executor

Calls external APIs on behalf of the agent.

- **Standardized interface** — every connector exposes: read(), write(), watch()
- **Auth management** — OAuth tokens stored encrypted, auto-refreshed
- **Error normalization** — Gmail 403 and Slack rate_limited both become a standard ToolError
- **Pagination** — auto-handles paginated APIs, returns complete results
- **Idempotency** — won't send the same email twice if a step retries

### 5. State Manager

Workflows aren't always instant. Some span hours or days.

- **Workflow memory** — "I already sent the first reminder, this is the second"
- **Timers** — "check back in 24 hours"
- **Queues** — pending approvals, waiting for external events
- **Checkpoints** — save state after each step, resume on failure
- **Per-customer isolation** — customer A's state never leaks to customer B

### 6. Security Layer

Not bolted on. Built in from day one.

- **Encryption** — prompts and outputs encrypted at rest (AES-256)
- **PII detection** — auto-redact SSN, email, phone before hitting model
- **Permissions** — each workflow scoped to specific tools and actions
- **Audit log** — immutable record of every decision and action
- **API key management** — scoped, rotatable, revocable

---

## Connector Interface

Every connector implements the same interface:

```python
class Connector:
    def authenticate(self, credentials):
        """OAuth flow or API key setup"""

    def read(self, resource, filters):
        """Get data — read email, get ticket, fetch invoice"""

    def write(self, resource, data):
        """Take action — send email, create ticket, update record"""

    def watch(self, event, callback):
        """Listen for triggers — new email, ticket created, payment received"""

    def schema(self):
        """Return what this connector can do — used by LLM to plan execution"""
```

Adding a new connector:
1. Implement these 4 methods
2. Register it with the framework
3. Done — the workflow engine can now use it

Target: a new connector takes 1-2 days to build, not weeks.

---

## SDK Design

### Python

```python
from agentframe import Agent, Workflow

# Initialize with any model
agent = Agent(model="vertex://llama-3.1-70b")

# Connect tools
agent.connect("gmail", credentials="oauth://...")
agent.connect("jira", credentials="token://...")
agent.connect("retell", credentials="apikey://...")

# Define workflow in plain English
agent.workflow("""
  When a Jira ticket is created in PHONE-AGENTS:
  1. Extract business details from the ticket
  2. Create a Retell agent with appropriate config
  3. Run test calls
  4. Update the Jira ticket with results
""")

# Or define structured tasks with typed I/O
@agent.task
def extract_invoice(email_body: str) -> InvoiceSchema:
    """Extract invoice details from this email."""

# Run as a service
agent.serve(port=8080)

# Or call directly
result = extract_invoice("Dear sir, please find attached...")
```

### TypeScript

```typescript
import { Agent, workflow } from 'agentframe';

const agent = new Agent({ model: 'vertex://llama-3.1-70b' });

agent.connect('gmail', { credentials: 'oauth://...' });
agent.connect('stripe', { credentials: 'apikey://...' });

agent.workflow(`
  When a new Stripe subscription is created:
  1. Look up customer in our database
  2. Send welcome email with onboarding steps
  3. Schedule 7-day check-in
`);

// Typed task
const categorize = agent.task<string, TicketCategory>({
  description: 'Categorize this support ticket',
  output: TicketCategorySchema,
});

agent.serve({ port: 8080 });
```

### CLI

```bash
$ agentframe init
$ agentframe connect gmail
$ agentframe connect jira

$ agentframe workflow create \
  --trigger "jira:ticket_created" \
  --steps "workflow.txt" \
  --approve-first-10

$ agentframe start
$ agentframe logs --follow
$ agentframe status
```

---

## Build Order

### Month 1: Core Engine

**Week 1-2: Model Router + Schema Enforcer**
- Vertex AI integration (Llama 70B)
- Ollama integration (local dev)
- OpenAI-compatible fallback
- Structured output with retry
- Input/output validation
- Cost tracking per call

**Week 3-4: Workflow Engine (basic)**
- Plain English → execution plan (LLM-powered parsing)
- Sequential step execution
- Context passing between steps
- Error handling and retry
- Basic logging

### Month 2: Tool System

**Week 5-6: Connector Framework + First 3 Connectors**
- Connector interface (read/write/watch)
- Auth management (OAuth + API key)
- Gmail connector
- Jira connector
- Generic REST connector (covers any API)

**Week 7-8: Workflow Engine (advanced)**
- Trigger system (webhooks, polling, scheduled)
- Branching (if/else based on LLM judgment)
- Human-in-the-loop approval
- Timer system ("follow up in 24 hours")
- Checkpointing and resume

### Month 3: Developer Experience

**Week 9-10: SDK + CLI**
- Python SDK (pip install agentframe)
- @agent.task decorator
- agent.workflow() plain English interface
- CLI for init, connect, deploy, logs
- 5-minute quickstart doc

**Week 11-12: Observability + Security**
- Dashboard: workflow runs, success/fail, costs
- Audit log
- Encryption at rest
- PII redaction
- Permission scoping

### Month 4: Ship It

**Week 13: Polish**
- Error messages that actually help
- Edge case handling
- Performance optimization
- Caching layer

**Week 14: Launch**
- GitHub repo (open source, MIT)
- Documentation site
- 3 example workflows:
  1. Jira → Retell agent creation (your story)
  2. Email → invoice extraction → QuickBooks
  3. GitHub PR → test → deploy → Slack notification
- Hacker News post
- Twitter/LinkedIn launch content

### Month 5-6: Growth

- More connectors based on demand
- TypeScript SDK
- Web dashboard (no-code interface for business owners)
- Hosted offering (managed Cloud Run deployment)
- Marketplace for workflow templates

---

## Infrastructure

### Development
```
Local: Ollama (free, instant)
CI: Vertex AI (pay per token, ~$0.20/1M)
```

### Production (hosted offering)
```
Model:    Vertex AI Llama 70B ($0.20/1M tokens)
Compute:  Cloud Run (serverless, scale to zero)
State:    Cloud SQL (PostgreSQL) or Firestore
Queue:    Cloud Tasks (timers, scheduled workflows)
Auth:     Firebase Auth or Clerk
Storage:  Cloud Storage (audit logs, checkpoints)
```

### Monthly cost at scale
```
100 users:    ~$50-100/mo
1,000 users:  ~$300-500/mo
10,000 users: ~$2,000-5,000/mo
```

---

## Revenue Model

### Free (open source)
- Framework + SDK
- All connectors
- Self-hosted with any model
- Community support

### Pro ($50/mo per developer)
- Hosted model endpoint (no GPU management)
- Dashboard and monitoring
- Email support

### Team ($200/mo)
- Everything in Pro
- Shared workflows
- Team permissions
- Priority model routing

### Business ($1,500/mo)
- No-code dashboard (the "stop hiring" product)
- Unlimited workflows
- Phone/chat support
- Custom connectors on request

### Enterprise ($5-10k/mo)
- Deployed in their cloud
- Their data never leaves their environment
- Custom model fine-tuning
- SLA and dedicated support
- SOC2 compliance package

---

## Success Metrics

### Month 1-2 (building)
- Engine handles 10 workflow types reliably
- 3 connectors working
- < 5 second end-to-end for simple workflows

### Month 3-4 (launch)
- 500+ GitHub stars
- 50+ developers using the SDK
- 3 example workflows people actually copy

### Month 5-6 (revenue)
- 10 paying Pro customers ($500 MRR)
- 5 paying Business customers ($7,500 MRR)
- 20+ community-contributed connectors

### Month 9-12 (scale)
- 1,000+ SDK users
- 50+ paying customers
- $50k+ MRR
- First enterprise deal
