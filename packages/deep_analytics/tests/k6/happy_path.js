import {
    CONFIG, createMCPClient, verifyMCPServerRunning, createTree,
    addLeaf, expandLeaf, navigateTo, exportPaths, inspectTree,
    balanceLeafs, validateCoherence
} from './shared_helpers.js';

export const options = {
    vus: CONFIG.DEFAULT_VUS,
    iterations: CONFIG.DEFAULT_ITERATIONS,
};

function testBasicTreeOperations(client) {
    console.log('\n=== Testing Basic Tree Operations ===');

    // Create main tree
    const { rootId } = createTree(
        client,
        "¿Cuál será el impacto de la IA en la economía global en 2030?",
        8
    );

    // Add initial branches under root
    const { leafId: automationId } = addLeaf(
        client,
        "Automatización masiva de empleos",
        "La IA reemplazará tareas rutinarias en múltiples sectores",
        0.8,
        9
    );

    const { leafId: businessModelsId } = addLeaf(
        client,
        "Nuevos modelos de negocio basados en IA",
        "Surgirán empresas que aprovechen la IA para crear valor",
        0.7,
        8
    );

    return { rootId, automationId, businessModelsId };
}

function testAdvancedTreeOperations(client, automationId) {
    console.log('\n=== Testing Advanced Tree Operations ===');

    // Expand automation branch
    expandLeaf(
        client,
        automationId,
        "Necesitamos analizar en detalle los sectores más afectados por la automatización"
    );

    // Add detailed sector analysis
    const { leafId: servicesId } = addLeaf(
        client,
        "Impacto en el sector servicios",
        "El sector servicios será uno de los más afectados por la automatización",
        0.75,
        8
    );

    const { leafId: manufacturingId } = addLeaf(
        client,
        "Impacto en el sector manufacturero",
        "La manufactura verá una transformación significativa por robots e IA",
        0.85,
        9
    );

    return { servicesId, manufacturingId };
}

function testNavigationAndDeepAnalysis(client, servicesId) {
    console.log('\n=== Testing Navigation and Deep Analysis ===');

    // Navigate to services sector for detailed analysis
    navigateTo(
        client,
        servicesId,
        "Analizaremos en detalle el impacto en servicios"
    );

    // Add specific service sector impacts
    const { leafId: customerServiceId } = addLeaf(
        client,
        "Automatización en atención al cliente",
        "Chatbots y asistentes virtuales reemplazarán roles de servicio",
        0.9,
        9
    );

    const { leafId: financialRolesId } = addLeaf(
        client,
        "Transformación de roles financieros",
        "La IA transformará la banca y servicios financieros",
        0.8,
        8
    );

    return { customerServiceId, financialRolesId };
}

function testTreeAnalysisFeatures(client) {
    console.log('\n=== Testing Tree Analysis Features ===');

    // Inspect tree structure
    inspectTree(client);

    // Balance probabilities
    balanceLeafs(client, "Conservative");

    // Validate coherence
    validateCoherence(
        client,
        "Este análisis evalúa la coherencia del árbol de decisión creado para analizar el impacto de la IA en la economía global. Cada rama representa un aspecto diferente del impacto económico."
    );
}

function testExportFunctionality(client) {
    console.log('\n=== Testing Export Functionality ===');

    // Export comprehensive analysis
    exportPaths(
        client,
        "Analytical",
        [
            "La automatización será más pronunciada en servicios y manufactura",
            "Se crearán nuevos modelos de negocio basados en IA que transformarán sectores tradicionales",
            "El impacto será especialmente fuerte en atención al cliente y roles financieros",
            "Los sectores que adopten IA temprano tendrán ventajas competitivas significativas",
            "Se requerirán nuevas políticas para gestionar la transición laboral"
        ],
        0.85
    );
}

function testCompleteWorkflow(client) {
    console.log('\n🚀 Starting Complete AI Economics Analysis Workflow');

    // Phase 1: Basic tree setup
    const { automationId } = testBasicTreeOperations(client);

    // Phase 2: Advanced analysis
    const { servicesId } = testAdvancedTreeOperations(client, automationId);

    // Phase 3: Deep sector analysis
    testNavigationAndDeepAnalysis(client, servicesId);

    // Phase 4: Analysis and validation
    testTreeAnalysisFeatures(client);

    // Phase 5: Export results
    testExportFunctionality(client);

    console.log('\n✅ Complete workflow executed successfully!');
}

export default function () {
    console.log('🔧 Deep Analytics MCP Server - Happy Path Test Suite');
    console.log('================================================');

    // Initialize client
    const client = createMCPClient();

    // Verify server is running
    if (!verifyMCPServerRunning(client)) {
        console.error('❌ MCP Server is not running. Aborting tests.');
        return;
    }

    // Run complete workflow
    testCompleteWorkflow(client);

    console.log('\n🎉 All happy path tests completed successfully!');
}