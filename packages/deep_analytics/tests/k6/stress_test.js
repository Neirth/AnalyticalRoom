import {
    CONFIG, createMCPClient, verifyMCPServerRunning, createTree,
    addLeaf, expandLeaf, navigateTo, exportPaths, balanceLeafs,
    validateCoherence, inspectTree, pruneTree
} from './shared_helpers.js';

export const options = {
    stages: [
        { duration: '30s', target: 2 },  // Ramp up to 2 users
        { duration: '1m', target: 2 },   // Stay at 2 users
        { duration: '30s', target: 5 },  // Ramp up to 5 users
        { duration: '1m', target: 5 },   // Stay at 5 users
        { duration: '30s', target: 0 },  // Ramp down to 0 users
    ],
    thresholds: {
        'checks': ['rate>0.95'], // 95% of checks should pass
        'http_req_duration': ['p(95)<2000'], // 95% of requests should be under 2s
    },
};

function createComplexTreeStructure(client, userId) {
    console.log(`🌳 User ${userId}: Creating complex tree structure`);

    // Create main analysis tree
    const { rootId } = createTree(
        client,
        `Usuario ${userId}: Análisis complejo de impacto tecnológico`,
        8
    );

    const branches = [];

    // Create multiple main branches
    for (let i = 1; i <= 3; i++) {
        const { leafId } = addLeaf(
            client,
            `Rama principal ${i} del usuario ${userId}`,
            `Análisis detallado de la rama número ${i}`,
            0.6 + (i * 0.1),
            7 + i
        );
        branches.push(leafId);
    }

    return { rootId, branches };
}

function expandBranchesRecursively(client, branches, userId, depth = 0) {
    if (depth >= 3) return; // Limit recursion depth

    console.log(`🔄 User ${userId}: Expanding branches at depth ${depth}`);

    branches.forEach((branchId, index) => {
        // Expand each branch
        expandLeaf(
            client,
            branchId,
            `Usuario ${userId}: Expansión nivel ${depth} rama ${index + 1}`
        );

        // Add sub-branches
        const subBranches = [];
        for (let i = 1; i <= 2; i++) {
            const { leafId } = addLeaf(
                client,
                `Sub-rama ${i} nivel ${depth} usuario ${userId}`,
                `Análisis detallado de sub-rama ${i} en profundidad ${depth}`,
                0.5 + (i * 0.1),
                5 + i
            );
            subBranches.push(leafId);
        }

        // Recursive expansion for next level
        if (subBranches.length > 0) {
            expandBranchesRecursively(client, subBranches, userId, depth + 1);
        }
    });
}

function performIntensiveAnalysis(client, userId) {
    console.log(`📊 User ${userId}: Performing intensive analysis`);

    // Multiple tree inspections
    for (let i = 0; i < 3; i++) {
        inspectTree(client);
    }

    // Multiple balance operations with different strategies
    ['Conservative', 'Neutral', 'Optimistic'].forEach(strategy => {
        balanceLeafs(client, strategy);
    });

    // Multiple coherence validations
    for (let i = 1; i <= 2; i++) {
        validateCoherence(
            client,
            `Usuario ${userId}: Validación de coherencia número ${i}. ` +
            `Este análisis detallado evalúa la consistencia lógica del árbol de decisión ` +
            `creado durante las pruebas de estrés. Se verifican las relaciones entre nodos, ` +
            `las probabilidades asignadas y la estructura general del análisis.`
        );
    }
}

function stressTestNavigation(client, userId) {
    console.log(`🧭 User ${userId}: Stress testing navigation`);

    // Create a tree for navigation testing
    const { rootId } = createTree(
        client,
        `Usuario ${userId}: Árbol para pruebas de navegación`,
        5
    );

    const nodes = [rootId];

    // Create a network of nodes
    for (let i = 1; i <= 10; i++) {
        const { leafId } = addLeaf(
            client,
            `Nodo de navegación ${i}`,
            `Nodo creado para pruebas de navegación intensiva`,
            0.5,
            5
        );
        nodes.push(leafId);
    }

    // Navigate rapidly between nodes
    for (let i = 0; i < 20; i++) {
        const randomNode = nodes[Math.floor(Math.random() * nodes.length)];
        navigateTo(
            client,
            randomNode,
            `Navegación aleatoria ${i + 1} del usuario ${userId}`
        );

        // Add a leaf after navigation
        addLeaf(
            client,
            `Hoja post-navegación ${i + 1}`,
            `Hoja añadida después de navegación número ${i + 1}`,
            Math.random(),
            Math.floor(Math.random() * 10) + 1
        );
    }
}

function performBulkExports(client, userId) {
    console.log(`📤 User ${userId}: Performing bulk exports`);

    const narrativeStyles = ['Analytical', 'Narrative', 'Technical'];

    for (let i = 0; i < 5; i++) {
        const style = narrativeStyles[i % narrativeStyles.length];

        exportPaths(
            client,
            style,
            [
                `Usuario ${userId}: Insight de exportación ${i + 1} parte A`,
                `Usuario ${userId}: Insight de exportación ${i + 1} parte B`,
                `Usuario ${userId}: Insight de exportación ${i + 1} parte C`,
                `Usuario ${userId}: Insight adicional para mayor volumen de datos`,
                `Usuario ${userId}: Insight final con información comprehensiva`
            ],
            0.7 + (Math.random() * 0.2)
        );
    }
}

function stressTestPruning(client, userId) {
    console.log(`✂️ User ${userId}: Stress testing pruning operations`);

    // Create a large tree for pruning
    const { rootId } = createTree(
        client,
        `Usuario ${userId}: Árbol grande para pruebas de poda`,
        9
    );

    // Add many nodes
    for (let i = 1; i <= 15; i++) {
        addLeaf(
            client,
            `Hoja para poda ${i}`,
            `Esta hoja será evaluada para poda. Número ${i} del usuario ${userId}`,
            Math.random(),
            Math.floor(Math.random() * 10) + 1
        );
    }

    // Perform multiple pruning operations
    const aggressiveness = [0.1, 0.3, 0.5, 0.7, 0.9];
    aggressiveness.forEach(level => {
        pruneTree(client, level);
    });
}

function runComprehensiveStressTest(client) {
    const userId = Math.floor(Math.random() * 1000);

    console.log(`\n🚀 Starting comprehensive stress test for user ${userId}`);

    try {
        // Phase 1: Complex tree creation
        const { branches } = createComplexTreeStructure(client, userId);

        // Phase 2: Recursive expansion
        expandBranchesRecursively(client, branches, userId);

        // Phase 3: Intensive analysis
        performIntensiveAnalysis(client, userId);

        // Phase 4: Navigation stress test
        stressTestNavigation(client, userId);

        // Phase 5: Bulk exports
        performBulkExports(client, userId);

        // Phase 6: Pruning stress test
        stressTestPruning(client, userId);

        console.log(`✅ User ${userId}: All stress test phases completed successfully`);

    } catch (error) {
        console.error(`❌ User ${userId}: Stress test failed:`, error.message);
        throw error;
    }
}

export default function () {
    console.log('💪 Deep Analytics MCP Server - Stress Test Suite');
    console.log('==============================================');

    // Initialize client
    const client = createMCPClient();

    // Verify server is running
    if (!verifyMCPServerRunning(client)) {
        console.error('❌ MCP Server is not running. Aborting stress tests.');
        return;
    }

    // Run comprehensive stress test
    runComprehensiveStressTest(client);

    console.log('\n🎯 Stress test iteration completed!');
}