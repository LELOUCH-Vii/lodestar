import { vi, describe, it, expect, beforeAll } from 'vitest';
import express from 'express';
import request from 'supertest';

const mockListAgents = vi.fn();
const mockGetAgent = vi.fn();
const mockGetAgentPolicy = vi.fn();
const mockGetAgentScore = vi.fn();
const mockGetAgentCount = vi.fn();
const mockIsAgentEligible = vi.fn();
const mockCheckSpendingAllowed = vi.fn();

vi.mock('../lib/contract.js', () => ({
  listAgents: (...args) => mockListAgents(...args),
  getAgent: (...args) => mockGetAgent(...args),
  getAgentPolicy: (...args) => mockGetAgentPolicy(...args),
  getAgentScore: (...args) => mockGetAgentScore(...args),
  getAgentCount: (...args) => mockGetAgentCount(...args),
  isAgentEligible: (...args) => mockIsAgentEligible(...args),
  checkSpendingAllowed: (...args) => mockCheckSpendingAllowed(...args),
  registerAgentOnChain: vi.fn(),
  recordPaymentOnChain: vi.fn(),
  flagAgentOnChain: vi.fn(),
  deactivateAgentOnChain: vi.fn(),
  updatePolicyOnChain: vi.fn(),
}));

vi.mock('../config.js', () => ({
  default: {
    contract: { agentsId: 'mock_agents_id' },
    server: { address: 'mock', secret: 'mock' },
    stellar: { network: 'testnet', rpcUrl: 'https://mock', networkPassphrase: 'mock', usdcContractId: 'mock' },
    x402: { facilitatorUrl: 'https://mock', searchPrice: '0.001', weatherPrice: '0.001' },
    braveApiKey: '',
    corsOrigin: ['http://localhost:3000'],
    jsonBodyLimit: '100kb',
    nodeEnv: 'test',
    port: 3001,
    logLevel: 'silent',
  },
}));

vi.mock('../lib/logger.js', () => ({
  default: { error: vi.fn(), info: vi.fn(), warn: vi.fn(), debug: vi.fn() },
}));

vi.mock('../middleware/ownerAuth.js', () => ({
  ownerAuth: (req, _res, next) => { req.callerAddress = 'GA MOCK'; next(); },
}));

vi.mock('../middleware/addressValidator.js', () => ({
  validateAgentAddressParam: (req, _res, next) => {
    if (req.params.address && req.params.address.startsWith('G')) {
      next();
    } else {
      _res.status(400).json({ error: 'Invalid address', code: 'INVALID_ADDRESS' });
    }
  },
  isValidStellarAddress: () => true,
}));

let app;

beforeAll(async () => {
  const router = (await import('./agents.js')).default;
  app = express();
  app.use(express.json());
  app.use('/api', router);
});

function makeAgent(overrides = {}) {
  return {
    address: 'GA7FYRB5CREWMDK2VIKVKWSW7V3YCCU3B3UHBJQ6JZ5OC7V7M5D4T8KJ',
    name: 'Test Agent',
    description: 'A test agent for testing',
    owner: 'GBV4ZDEPLQTVPQFJRME2BGYL6VQJLTTRIWJHTJNFGXBVF4WY5DJIQK2K',
    score: 100,
    total_payments: 5,
    successful_payments: 3,
    failed_payments: 2,
    total_volume_stroops: '10000000',
    registered_at: 1000,
    last_active: 2000,
    active: true,
    flagged: false,
    flag_reason: '',
    ...overrides,
  };
}

describe('GET /api/agents', () => {
  it('should return list of agents', async () => {
    const agents = [makeAgent({ address: 'GA1' }), makeAgent({ address: 'GA2' })];
    mockListAgents.mockResolvedValueOnce(agents);

    const res = await request(app).get('/api/agents');

    expect(res.status).toBe(200);
    expect(res.body.agents).toHaveLength(2);
    expect(res.body.count).toBe(2);
  });

  it('should return 500 when contract call fails', async () => {
    mockListAgents.mockRejectedValueOnce(new Error('Chain error'));

    const res = await request(app).get('/api/agents');

    expect(res.status).toBe(500);
    expect(res.body).toEqual({ error: 'Failed to fetch agents', code: 'FETCH_ERROR' });
  });
});

describe('GET /api/agents/count', () => {
  it('should return agent count', async () => {
    mockGetAgentCount.mockResolvedValueOnce(5);

    const res = await request(app).get('/api/agents/count');

    expect(res.status).toBe(200);
    expect(res.body.count).toBe(5);
  });
});

describe('GET /api/agents/stats', () => {
  it('should return stats for agents', async () => {
    const agents = [
      makeAgent({ score: 100, total_volume_stroops: '10000000' }),
      makeAgent({ score: 200, total_volume_stroops: '20000000' }),
    ];
    mockListAgents.mockResolvedValueOnce(agents);

    const res = await request(app).get('/api/agents/stats');

    expect(res.status).toBe(200);
    expect(res.body.totalAgents).toBe(2);
    expect(res.body.avgScore).toBe(150);
  });

  it('should return zero stats when no agents', async () => {
    mockListAgents.mockResolvedValueOnce([]);

    const res = await request(app).get('/api/agents/stats');

    expect(res.status).toBe(200);
    expect(res.body.totalAgents).toBe(0);
    expect(res.body.avgScore).toBe(0);
  });
});

describe('GET /api/agents/:address', () => {
  it('should return agent with policy', async () => {
    const agent = makeAgent();
    mockGetAgent.mockResolvedValueOnce(agent);
    mockGetAgentPolicy.mockResolvedValueOnce(null);

    const res = await request(app).get('/api/agents/GA7FYRB5CREWMDK2VIKVKWSW7V3YCCU3B3UHBJQ6JZ5OC7V7M5D4T8KJ');

    expect(res.status).toBe(200);
    expect(res.body.agent.name).toBe('Test Agent');
  });

  it('should return 404 if agent not found', async () => {
    mockGetAgent.mockResolvedValueOnce(null);

    const res = await request(app).get('/api/agents/GA7FYRB5CREWMDK2VIKVKWSW7V3YCCU3B3UHBJQ6JZ5OC7V7M5D4T8KJ');

    expect(res.status).toBe(404);
    expect(res.body.error).toBe('Agent not found');
  });
});

describe('GET /api/agents/:address/score', () => {
  it('should return agent score', async () => {
    mockGetAgentScore.mockResolvedValueOnce(85);

    const res = await request(app).get('/api/agents/GA7FYRB5CREWMDK2VIKVKWSW7V3YCCU3B3UHBJQ6JZ5OC7V7M5D4T8KJ/score');

    expect(res.status).toBe(200);
    expect(res.body.score).toBe(85);
  });
});
