import {
    CONFIG, createMCPClient, verifyMCPServerRunning, createTree,
    addLeaf, expandLeaf, navigateTo, exportPaths, balanceLeafs,
    validateCoherence, pruneTree, expectError
} from './shared_helpers.js';

export const options = {
    vus: CONFIG.DEFAULT_VUS,
    iterations: CONFIG.DEFAULT_ITERATIONS,
};

function testOperationsWithoutTree(client) {
    console.log('\n=== Testing Operations Without Tree ===');

    // Test adding leaf without tree
    expectError(() => addLeaf(
        client,
        "Esta hoja no debería crearse",
        "No hay árbol",
        0.5,
        5,
        false
    ), 'No cursor set');

    // Test navigation without tree
    expectError(() => navigateTo(
        client,
        "fake-node-id",
        "Navegación sin árbol",
        false
    ), 'Node not found');

    // Test expand without tree
    expectError(() => expandLeaf(
        client,
        "fake-node-id",
        "Expansión sin árbol",
        false
    ), 'Node not found');
}

function testInvalidTreeCreation(client) {
    console.log('\n=== Testing Invalid Tree Creation ===');

    // Test complexity too high
    expectError(() => createTree(
        client,
        "Árbol con complejidad muy alta",
        15 // Maximum is 10
    ), 'Invalid complexity');

    // Test complexity too low
    expectError(() => createTree(
        client,
        "Árbol con complejidad muy baja",
        0 // Minimum is 1
    ), 'Invalid complexity');

    // Test extremely short premise
    expectError(() => createTree(
        client,
        "X",
        5
    ), 'Premise too short');
}

function testInvalidLeafOperations(client) {
    console.log('\n=== Testing Invalid Leaf Operations ===');

    // Create a valid tree first
    const { rootId } = createTree(
        client,
        "Árbol para probar casos límite",
        5
    );

    // Test invalid probabilities
    addLeaf(
        client,
        "Hoja con probabilidad muy alta",
        "La probabilidad debe estar entre 0 y 1",
        1.5,
        5,
        false
    );

    addLeaf(
        client,
        "Hoja con probabilidad negativa",
        "La probabilidad no puede ser negativa",
        -0.1,
        5,
        false
    );

    // Test invalid confidence levels
    addLeaf(
        client,
        "Hoja con confianza muy alta",
        "La confianza debe estar entre 1 y 10",
        0.5,
        15,
        false
    );

    addLeaf(
        client,
        "Hoja con confianza muy baja",
        "La confianza debe estar entre 1 y 10",
        0.5,
        0,
        false
    );

    // Test empty premise and reasoning
    addLeaf(
        client,
        "",
        "Razonamiento sin premisa",
        0.5,
        5,
        false
    );

    addLeaf(
        client,
        "Premisa sin razonamiento",
        "",
        0.5,
        5,
        false
    );

    return rootId;
}

function testInvalidExpansionOperations(client, rootId) {
    console.log('\n=== Testing Invalid Expansion Operations ===');

    // Create a valid leaf first
    const { leafId } = addLeaf(
        client,
        "Hoja para probar expansión",
        "Esta hoja la expandiremos y luego intentaremos expandir de nuevo",
        0.5,
        5
    );

    // First expansion should work
    expandLeaf(
        client,
        leafId,
        "Primera expansión válida"
    );

    // Second expansion should fail (no longer a leaf)
    expandLeaf(
        client,
        leafId,
        "Segunda expansión que debería fallar",
        false
    );

    // Test expansion of non-existent node
    expandLeaf(
        client,
        "node-that-does-not-exist-12345",
        "Expansión de nodo inexistente",
        false
    );

    // Test expansion of root node
    expandLeaf(
        client,
        rootId,
        "Intentar expandir el nodo raíz",
        false
    );

    return leafId;
}

function testInvalidNavigationOperations(client) {
    console.log('\n=== Testing Invalid Navigation Operations ===');

    // Test navigation to non-existent node
    navigateTo(
        client,
        "non-existent-node-id-xyz",
        "Navegación a nodo inexistente",
        false
    );

    // Test navigation with empty justification
    navigateTo(
        client,
        "any-node-id",
        "",
        false
    );
}

function testInvalidExportOperations(client) {
    console.log('\n=== Testing Invalid Export Operations ===');

    // Test export with insufficient insights
    exportPaths(
        client,
        "Analytical",
        ["Solo un insight"], // Need at least 3
        0.5,
        false
    );

    // Test export with empty insights
    exportPaths(
        client,
        "Analytical",
        [],
        0.5,
        false
    );

    // Test export with invalid confidence
    exportPaths(
        client,
        "Analytical",
        ["Insight 1", "Insight 2", "Insight 3"],
        1.5, // > 1.0
        false
    );

    exportPaths(
        client,
        "Analytical",
        ["Insight 1", "Insight 2", "Insight 3"],
        -0.1, // < 0.0
        false
    );

    // Test export with invalid narrative style
    exportPaths(
        client,
        "EstiloInexistente",
        ["Insight 1", "Insight 2", "Insight 3"],
        0.5,
        false
    );
}

function testInvalidAnalysisOperations(client) {
    console.log('\n=== Testing Invalid Analysis Operations ===');

    // Test balance with invalid uncertainty type
    balanceLeafs(
        client,
        "TipoInvalido",
        false
    );

    // Test coherence validation with short analysis
    validateCoherence(
        client,
        "Corto", // Too short
        false
    );

    // Test pruning with invalid aggressiveness
    pruneTree(
        client,
        1.5, // > 1.0
        false
    );

    pruneTree(
        client,
        -0.1, // < 0.0
        false
    );
}

function testEdgeCasesWithValidData(client) {
    console.log('\n=== Testing Edge Cases with Valid Data ===');

    // Create tree with minimum complexity
    const { rootId: minTreeId } = createTree(
        client,
        "Árbol con complejidad mínima para testing de límites",
        1
    );

    // Create tree with maximum complexity
    const { rootId: maxTreeId } = createTree(
        client,
        "Árbol con complejidad máxima para verificar que funciona correctamente",
        10
    );

    // Add leaf with minimum probability
    addLeaf(
        client,
        "Evento muy improbable",
        "Este evento tiene la probabilidad mínima posible sin ser imposible",
        0.01,
        1
    );

    // Add leaf with maximum probability
    addLeaf(
        client,
        "Evento casi seguro",
        "Este evento tiene muy alta probabilidad de ocurrir",
        0.99,
        10
    );

    // Test with very long premise and reasoning
    addLeaf(
        client,
        "Esta es una premisa extremadamente larga que pretende probar los límites del sistema y verificar que puede manejar textos extensos sin problemas de procesamiento o almacenamiento",
        "Este es un razonamiento igualmente largo que explica detalladamente las razones detrás de esta premisa extensa, incluyendo múltiples factores, consideraciones, análisis profundos y justificaciones comprehensivas que demuestran la robustez del sistema",
        0.5,
        5
    );
}

function testBoundaryConditions(client) {
    console.log('\n=== Testing Boundary Conditions ===');

    // Create a tree for boundary testing
    createTree(
        client,
        "Árbol para probar condiciones límite",
        5
    );

    // Test rapid successive operations
    for (let i = 0; i < 5; i++) {
        addLeaf(
            client,
            `Hoja rápida ${i + 1}`,
            `Creada en secuencia rápida número ${i + 1}`,
            0.5,
            5
        );
    }

    // Test balance after rapid additions
    balanceLeafs(client, "Neutral");

    // Test export with exactly minimum required insights
    exportPaths(
        client,
        "Analytical",
        [
            "Primer insight mínimo requerido",
            "Segundo insight para cumplir requisitos",
            "Tercer insight para completar el mínimo"
        ],
        0.5
    );
}

export default function () {
    console.log('🧪 Deep Analytics MCP Server - Edge Cases Test Suite');
    console.log('==================================================');

    // Initialize client
    const client = createMCPClient();

    // Verify server is running
    if (!verifyMCPServerRunning(client)) {
        console.error('❌ MCP Server is not running. Aborting tests.');
        return;
    }

    // Run all edge case tests
    testOperationsWithoutTree(client);
    testInvalidTreeCreation(client);
    const rootId = testInvalidLeafOperations(client);
    const leafId = testInvalidExpansionOperations(client, rootId);
    testInvalidNavigationOperations(client);
    testInvalidExportOperations(client);
    testInvalidAnalysisOperations(client);
    testEdgeCasesWithValidData(client);
    testBoundaryConditions(client);

    console.log('\n🎯 All edge case tests completed successfully!');
    console.log('📊 Server correctly handled all invalid inputs and boundary conditions.');
}