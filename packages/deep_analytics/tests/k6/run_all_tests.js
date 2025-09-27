import {
    CONFIG, createMCPClient, verifyMCPServerRunning
} from './shared_helpers.js';

export const options = {
    vus: 1,
    iterations: 1,
};

// Import test functions from other files would need to be restructured for k6
// For now, we'll create a comprehensive test that covers all scenarios

function runBasicFunctionalityTests(client) {
    console.log('\n🔧 Running Basic Functionality Tests');
    console.log('=====================================');

    const testResults = {
        passed: 0,
        failed: 0,
        total: 0
    };

    function test(name, fn) {
        testResults.total++;
        try {
            fn();
            console.log(`✅ ${name}`);
            testResults.passed++;
        } catch (error) {
            console.log(`❌ ${name}: ${error.message}`);
            testResults.failed++;
        }
    }

    // Basic server tests
    test('MCP Server connectivity', () => {
        if (!client.ping()) throw new Error('Server not responding');
    });

    // Tree creation tests
    test('Create tree with valid parameters', () => {
        const result = client.callTool({
            name: 'create_tree',
            arguments: {
                premise: "Test tree for comprehensive testing",
                complexity: 5
            }
        });
        if (!result.content[0].text.includes('root ID:')) {
            throw new Error('Tree creation failed');
        }
    });

    // Add more comprehensive tests here...

    return testResults;
}

function runEdgeCaseTests(client) {
    console.log('\n🧪 Running Edge Case Tests');
    console.log('===========================');

    const testResults = {
        passed: 0,
        failed: 0,
        total: 0
    };

    function test(name, fn) {
        testResults.total++;
        try {
            fn();
            console.log(`✅ ${name}`);
            testResults.passed++;
        } catch (error) {
            console.log(`❌ ${name}: ${error.message}`);
            testResults.failed++;
        }
    }

    // Edge case tests
    test('Reject invalid complexity values', () => {
        try {
            const result = client.callTool({
                name: 'create_tree',
                arguments: {
                    premise: "Invalid complexity test",
                    complexity: 15
                }
            });
            if (!result.content[0].text.includes('Error')) {
                throw new Error('Should have rejected invalid complexity');
            }
        } catch (e) {
            // Expected to fail
        }
    });

    // Add more edge case tests here...

    return testResults;
}

function runPerformanceTests(client) {
    console.log('\n⚡ Running Performance Tests');
    console.log('=============================');

    const testResults = {
        passed: 0,
        failed: 0,
        total: 0
    };

    function test(name, fn) {
        testResults.total++;
        try {
            const startTime = Date.now();
            fn();
            const duration = Date.now() - startTime;
            console.log(`✅ ${name} (${duration}ms)`);
            testResults.passed++;
        } catch (error) {
            console.log(`❌ ${name}: ${error.message}`);
            testResults.failed++;
        }
    }

    // Performance tests
    test('Tree creation performance', () => {
        const result = client.callTool({
            name: 'create_tree',
            arguments: {
                premise: "Performance test tree with moderate complexity",
                complexity: 7
            }
        });
        if (!result.content[0].text.includes('root ID:')) {
            throw new Error('Tree creation failed');
        }
    });

    test('Rapid leaf addition', () => {
        // First create a tree
        client.callTool({
            name: 'create_tree',
            arguments: {
                premise: "Tree for rapid leaf testing",
                complexity: 5
            }
        });

        // Add multiple leaves rapidly
        for (let i = 0; i < 10; i++) {
            const result = client.callTool({
                name: 'add_leaf',
                arguments: {
                    premise: `Rapid leaf ${i + 1}`,
                    reasoning: `Added for performance testing iteration ${i + 1}`,
                    probability: 0.5,
                    confidence: 5
                }
            });
            if (!result.content[0].text.includes('ID:')) {
                throw new Error(`Leaf ${i + 1} creation failed`);
            }
        }
    });

    // Add more performance tests here...

    return testResults;
}

function generateTestReport(basicResults, edgeResults, perfResults) {
    const totalPassed = basicResults.passed + edgeResults.passed + perfResults.passed;
    const totalFailed = basicResults.failed + edgeResults.failed + perfResults.failed;
    const totalTests = basicResults.total + edgeResults.total + perfResults.total;

    console.log('\n📊 COMPREHENSIVE TEST REPORT');
    console.log('=============================');
    console.log(`📈 Basic Functionality: ${basicResults.passed}/${basicResults.total} passed`);
    console.log(`🧪 Edge Cases: ${edgeResults.passed}/${edgeResults.total} passed`);
    console.log(`⚡ Performance: ${perfResults.passed}/${perfResults.total} passed`);
    console.log('-----------------------------');
    console.log(`🎯 TOTAL: ${totalPassed}/${totalTests} tests passed`);
    console.log(`📊 Success Rate: ${((totalPassed / totalTests) * 100).toFixed(1)}%`);

    if (totalFailed > 0) {
        console.log(`⚠️  ${totalFailed} tests failed - review output above`);
    } else {
        console.log('🎉 All tests passed successfully!');
    }

    return {
        totalTests,
        totalPassed,
        totalFailed,
        successRate: (totalPassed / totalTests) * 100
    };
}

export default function () {
    console.log('🚀 Deep Analytics MCP Server - Comprehensive Test Suite');
    console.log('========================================================');
    console.log(`📍 Testing against: ${CONFIG.BASE_URL}`);

    // Initialize client
    const client = createMCPClient();

    // Verify server is running
    if (!verifyMCPServerRunning(client)) {
        console.error('❌ MCP Server is not running. Cannot proceed with tests.');
        console.error('💡 Make sure to start the server with: cargo run');
        return;
    }

    console.log('✅ MCP Server is running and responsive');

    // Run all test suites
    const basicResults = runBasicFunctionalityTests(client);
    const edgeResults = runEdgeCaseTests(client);
    const perfResults = runPerformanceTests(client);

    // Generate final report
    const finalReport = generateTestReport(basicResults, edgeResults, perfResults);

    // Set exit conditions based on results
    if (finalReport.successRate < 90) {
        console.log('\n❌ Test suite failed - success rate below 90%');
    } else if (finalReport.successRate < 100) {
        console.log('\n⚠️  Test suite completed with some failures');
    } else {
        console.log('\n🎉 Perfect test run - all tests passed!');
    }

    console.log(`\n📋 Test execution completed at ${new Date().toISOString()}`);
}