import mcp from 'k6/x/mcp';
import { check } from 'k6';

export const CONFIG = {
    BASE_URL: 'http://localhost:8080/mcp',
    DEFAULT_VUS: 1,
    DEFAULT_ITERATIONS: 1
};

export function createMCPClient() {
    return new mcp.StreamableHTTPClient({
        base_url: CONFIG.BASE_URL,
    });
}

export function extractIdFromResponse(response, idType = 'ID') {
    const patterns = {
        'ID': /ID: ([a-f0-9-]+)/,
        'root ID': /root ID: ([a-f0-9-]+)/
    };

    const pattern = patterns[idType];
    const match = response.content[0].text.match(pattern);
    return match ? match[1] : null;
}

export function verifyMCPServerRunning(client) {
    const isRunning = client.ping();
    check(isRunning, { 'MCP server is running': (running) => running === true });
    console.log('MCP server running:', isRunning);
    return isRunning;
}

export function createTree(client, premise, complexity = 8) {
    console.log(`Creating tree: "${premise}" with complexity ${complexity}`);

    const result = client.callTool({
        name: 'create_tree',
        arguments: { premise, complexity }
    });

    const rootId = extractIdFromResponse(result, 'root ID');
    check(rootId, { 'Tree created successfully': (id) => id !== null });

    console.log(`✓ Tree created with root ID: ${rootId}`);
    return { result, rootId };
}

export function addLeaf(client, premise, reasoning, probability, confidence, expectedToSucceed = true) {
    console.log(`Adding leaf: "${premise}" (prob: ${probability}, conf: ${confidence})`);

    const result = client.callTool({
        name: 'add_leaf',
        arguments: { premise, reasoning, probability, confidence }
    });

    if (expectedToSucceed) {
        const leafId = extractIdFromResponse(result);
        check(leafId, { [`Leaf "${premise}" created successfully`]: (id) => id !== null });
        console.log(`✓ Leaf created with ID: ${leafId}`);
        return { result, leafId };
    } else {
        check(result.content[0].text, {
            [`Adding "${premise}" failed as expected`]: (text) => text.includes('Error') || text.includes('error')
        });
        console.log(`✓ Leaf creation failed as expected`);
        return { result, leafId: null };
    }
}

export function expandLeaf(client, nodeId, rationale, expectedToSucceed = true) {
    console.log(`Expanding leaf: ${nodeId} with rationale: "${rationale}"`);

    const result = client.callTool({
        name: 'expand_leaf',
        arguments: { node_id: nodeId, rationale }
    });

    if (expectedToSucceed) {
        check(result.content[0].text, {
            'Leaf expanded successfully': (text) => !text.includes('Error') && !text.includes('error')
        });
        console.log(`✓ Leaf expanded successfully`);
    } else {
        check(result.content[0].text, {
            'Leaf expansion failed as expected': (text) => text.includes('Error') || text.includes('error')
        });
        console.log(`✓ Leaf expansion failed as expected`);
    }

    return result;
}

export function navigateTo(client, nodeId, justification, expectedToSucceed = true) {
    console.log(`Navigating to: ${nodeId} with justification: "${justification}"`);

    const result = client.callTool({
        name: 'navigate_to',
        arguments: { node_id: nodeId, justification }
    });

    if (expectedToSucceed) {
        check(result.content[0].text, {
            'Navigation successful': (text) => !text.includes('Error') && !text.includes('error')
        });
        console.log(`✓ Navigation successful`);
    } else {
        check(result.content[0].text, {
            'Navigation failed as expected': (text) => text.includes('Error') || text.includes('error')
        });
        console.log(`✓ Navigation failed as expected`);
    }

    return result;
}

export function exportPaths(client, narrativeStyle, insights, confidenceAssessment, expectedToSucceed = true) {
    console.log(`Exporting paths with ${insights.length} insights and confidence ${confidenceAssessment}`);

    const result = client.callTool({
        name: 'export_paths',
        arguments: {
            narrative_style: narrativeStyle,
            insights,
            confidence_assessment: confidenceAssessment
        }
    });

    if (expectedToSucceed) {
        check(result.content[0].text, {
            'Export completed successfully': (text) => text.includes('Analysis exported') || text.includes('exported')
        });
        console.log(`✓ Export completed successfully`);
    } else {
        check(result.content[0].text, {
            'Export failed as expected': (text) => text.includes('Error') || text.includes('error')
        });
        console.log(`✓ Export failed as expected`);
    }

    return result;
}

export function balanceLeafs(client, uncertaintyType, expectedToSucceed = true) {
    console.log(`Balancing leafs with uncertainty type: ${uncertaintyType}`);

    const result = client.callTool({
        name: 'balance_leafs',
        arguments: { uncertainty_type: uncertaintyType }
    });

    if (expectedToSucceed) {
        check(result.content[0].text, {
            'Balance completed successfully': (text) => !text.includes('Error') && !text.includes('error')
        });
        console.log(`✓ Balance completed successfully`);
    } else {
        check(result.content[0].text, {
            'Balance failed as expected': (text) => text.includes('Error') || text.includes('error')
        });
        console.log(`✓ Balance failed as expected`);
    }

    return result;
}

export function validateCoherence(client, analysisDetail, expectedToSucceed = true) {
    console.log(`Validating coherence with analysis: "${analysisDetail.substring(0, 50)}..."`);

    const result = client.callTool({
        name: 'validate_coherence',
        arguments: { analysis_detail: analysisDetail }
    });

    if (expectedToSucceed) {
        check(result.content[0].text, {
            'Coherence validation successful': (text) => !text.includes('Error') && !text.includes('error')
        });
        console.log(`✓ Coherence validation successful`);
    } else {
        check(result.content[0].text, {
            'Coherence validation failed as expected': (text) => text.includes('Error') || text.includes('error')
        });
        console.log(`✓ Coherence validation failed as expected`);
    }

    return result;
}

export function inspectTree(client) {
    console.log('Inspecting tree structure');

    const result = client.callTool({
        name: 'inspect_tree',
        arguments: {}
    });

    check(result.content[0].text, {
        'Tree inspection successful': (text) => !text.includes('Error') && !text.includes('error')
    });

    console.log(`✓ Tree inspection completed`);
    return result;
}

export function pruneTree(client, aggressiveness, expectedToSucceed = true) {
    console.log(`Pruning tree with aggressiveness: ${aggressiveness}`);

    const result = client.callTool({
        name: 'prune_tree',
        arguments: { aggressiveness }
    });

    if (expectedToSucceed) {
        check(result.content[0].text, {
            'Pruning successful': (text) => !text.includes('Error') && !text.includes('error')
        });
        console.log(`✓ Pruning completed successfully`);
    } else {
        check(result.content[0].text, {
            'Pruning failed as expected': (text) => text.includes('Error') || text.includes('error')
        });
        console.log(`✓ Pruning failed as expected`);
    }

    return result;
}

export function expectError(operation, expectedError) {
    try {
        const result = operation();
        check(result.content[0].text, {
            [`Expected error: ${expectedError}`]: (text) => text.includes('Error') || text.includes('error')
        });
        return result;
    } catch (e) {
        console.log(`Expected error caught: ${e.message}`);
        check(true, { [`Error caught as expected: ${expectedError}`]: () => true });
        return null;
    }
}