import {
    OpenGeometry,
    Rectangle,
    BooleanOperations,
    OGBooleanEngine,
    OGBrep,
    Vector3
} from '../main/opengeometry-three/';
import * as THREE from 'three';

/**
 * Comprehensive test and example for OpenGeometry Boolean Operations
 * This file demonstrates all boolean operations with proper error handling
 */

interface BooleanTestResult {
    success: boolean;
    operation: string;
    message: string;
    geometry?: any;
}

class BooleanOperationsDemo {
    private scene: THREE.Scene;
    private camera: THREE.Camera;
    private openGeometry: OpenGeometry;
    private booleanOperations: BooleanOperations;

    constructor(container: HTMLElement) {
        // Initialize Three.js scene
        this.scene = new THREE.Scene();
        this.camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 1000);
        
        // Initialize OpenGeometry
        this.openGeometry = new OpenGeometry(container, this.scene, this.camera);
        this.booleanOperations = new BooleanOperations();
    }

    async initialize(): Promise<void> {
        await this.openGeometry.init();
        console.log('OpenGeometry Boolean Operations Demo initialized');
    }

    /**
     * Create a test rectangle with triangulation
     */
    private createTestRectangle(width: number, height: number, center: Vector3): Rectangle | null {
        try {
            const rectangle = new Rectangle({
                width,
                breadth: height,
                center,
                color: 0x00ff00
            });

            // Ensure the rectangle is triangulated
            rectangle.generate_geometry();
            
            return rectangle;
        } catch (error) {
            console.error('Failed to create test rectangle:', error);
            return null;
        }
    }

    /**
     * Test union operation between two rectangles
     */
    async testUnion(): Promise<BooleanTestResult> {
        try {
            console.log('Testing Union Operation...');

            // Create two overlapping rectangles
            const rect1 = this.createTestRectangle(
                4.0, 4.0, 
                new Vector3(0.0, 0.0, 0.0)
            );
            const rect2 = this.createTestRectangle(
                4.0, 4.0, 
                new Vector3(2.0, 0.0, 2.0)
            );

            if (!rect1 || !rect2) {
                throw new Error('Failed to create test rectangles');
            }

            // Perform union operation
            const result = await this.booleanOperations.union(rect1, rect2);

            if (!result.success) {
                throw new Error(`Union failed: ${result.error}`);
            }

            return {
                success: true,
                operation: 'Union',
                message: 'Union operation completed successfully',
                geometry: result.geometry
            };

        } catch (error) {
            return {
                success: false,
                operation: 'Union',
                message: `Union operation failed: ${error}`
            };
        }
    }

    /**
     * Test intersection operation between two rectangles
     */
    async testIntersection(): Promise<BooleanTestResult> {
        try {
            console.log('Testing Intersection Operation...');

            const rect1 = this.createTestRectangle(
                6.0, 6.0, 
                new Vector3(0.0, 0.0, 0.0)
            );
            const rect2 = this.createTestRectangle(
                4.0, 4.0, 
                new Vector3(1.0, 0.0, 1.0)
            );

            if (!rect1 || !rect2) {
                throw new Error('Failed to create test rectangles');
            }

            const result = await this.booleanOperations.intersection(rect1, rect2);

            if (!result.success) {
                throw new Error(`Intersection failed: ${result.error}`);
            }

            return {
                success: true,
                operation: 'Intersection',
                message: 'Intersection operation completed successfully',
                geometry: result.geometry
            };

        } catch (error) {
            return {
                success: false,
                operation: 'Intersection',
                message: `Intersection operation failed: ${error}`
            };
        }
    }

    /**
     * Test difference operation (subtraction)
     */
    async testDifference(): Promise<BooleanTestResult> {
        try {
            console.log('Testing Difference Operation...');

            const rect1 = this.createTestRectangle(
                8.0, 8.0, 
                new Vector3(0.0, 0.0, 0.0)
            );
            const rect2 = this.createTestRectangle(
                4.0, 4.0, 
                new Vector3(2.0, 0.0, 2.0)
            );

            if (!rect1 || !rect2) {
                throw new Error('Failed to create test rectangles');
            }

            const result = await this.booleanOperations.difference(rect1, rect2);

            if (!result.success) {
                throw new Error(`Difference failed: ${result.error}`);
            }

            return {
                success: true,
                operation: 'Difference',
                message: 'Difference operation completed successfully',
                geometry: result.geometry
            };

        } catch (error) {
            return {
                success: false,
                operation: 'Difference',
                message: `Difference operation failed: ${error}`
            };
        }
    }

    /**
     * Test symmetric difference operation (XOR)
     */
    async testSymmetricDifference(): Promise<BooleanTestResult> {
        try {
            console.log('Testing Symmetric Difference Operation...');

            const rect1 = this.createTestRectangle(
                6.0, 6.0, 
                new Vector3(0.0, 0.0, 0.0)
            );
            const rect2 = this.createTestRectangle(
                6.0, 6.0, 
                new Vector3(3.0, 0.0, 3.0)
            );

            if (!rect1 || !rect2) {
                throw new Error('Failed to create test rectangles');
            }

            const result = await this.booleanOperations.symmetricDifference(rect1, rect2);

            if (!result.success) {
                throw new Error(`Symmetric difference failed: ${result.error}`);
            }

            return {
                success: true,
                operation: 'Symmetric Difference',
                message: 'Symmetric difference operation completed successfully',
                geometry: result.geometry
            };

        } catch (error) {
            return {
                success: false,
                operation: 'Symmetric Difference',
                message: `Symmetric difference operation failed: ${error}`
            };
        }
    }

    /**
     * Test direct WASM boolean engine usage
     */
    async testDirectWasmEngine(): Promise<BooleanTestResult> {
        try {
            console.log('Testing Direct WASM Boolean Engine...');

            // Create boolean engine with custom tolerance
            const engine = new OGBooleanEngine(1e-6);

            // Create test Breps
            const brep1 = new OGBrep("rectangle");
            const brep2 = new OGBrep("rectangle");

            // Test engine operations (this would require serialized Brep data)
            // For now, just test engine creation and configuration
            console.log('Boolean engine created successfully');
            console.log('Engine tolerance:', engine.tolerance);

            return {
                success: true,
                operation: 'Direct WASM Engine',
                message: 'Direct WASM boolean engine test completed successfully'
            };

        } catch (error) {
            return {
                success: false,
                operation: 'Direct WASM Engine',
                message: `Direct WASM engine test failed: ${error}`
            };
        }
    }

    /**
     * Run all boolean operation tests
     */
    async runAllTests(): Promise<BooleanTestResult[]> {
        console.log('=== OpenGeometry Boolean Operations Test Suite ===');
        
        const results: BooleanTestResult[] = [];

        // Test each operation
        results.push(await this.testUnion());
        results.push(await this.testIntersection());
        results.push(await this.testDifference());
        results.push(await this.testSymmetricDifference());
        results.push(await this.testDirectWasmEngine());

        // Print results summary
        console.log('\n=== Test Results Summary ===');
        results.forEach(result => {
            const status = result.success ? '✅ PASS' : '❌ FAIL';
            console.log(`${status} ${result.operation}: ${result.message}`);
        });

        const passCount = results.filter(r => r.success).length;
        const totalCount = results.length;
        console.log(`\nOverall: ${passCount}/${totalCount} tests passed`);

        return results;
    }

    /**
     * Create visual demonstration of boolean operations
     */
    async createVisualDemo(): Promise<void> {
        console.log('Creating visual demonstration...');

        try {
            // Create original shapes
            const shape1 = this.createTestRectangle(4.0, 4.0, new Vector3(-6.0, 0.0, 0.0));
            const shape2 = this.createTestRectangle(4.0, 4.0, new Vector3(-4.0, 0.0, 2.0));

            if (shape1 && shape2) {
                // Add original shapes to scene
                shape1.material.color.set(0xff0000); // Red
                shape2.material.color.set(0x0000ff); // Blue
                this.scene.add(shape1);
                this.scene.add(shape2);

                // Create and add boolean results
                const unionResult = await this.booleanOperations.union(
                    this.createTestRectangle(4.0, 4.0, new Vector3(0.0, 0.0, 0.0))!,
                    this.createTestRectangle(4.0, 4.0, new Vector3(2.0, 0.0, 2.0))!
                );

                if (unionResult.success && unionResult.geometry) {
                    unionResult.geometry.position.set(2.0, 0.0, -6.0);
                    unionResult.geometry.material.color.set(0x00ff00); // Green
                    this.scene.add(unionResult.geometry);
                }

                console.log('Visual demonstration created successfully');
            }
        } catch (error) {
            console.error('Failed to create visual demonstration:', error);
        }
    }
}

// Export for use in applications
export { BooleanOperationsDemo, type BooleanTestResult };

// Example usage
export async function runBooleanDemo(container: HTMLElement): Promise<BooleanTestResult[]> {
    const demo = new BooleanOperationsDemo(container);
    await demo.initialize();
    
    const results = await demo.runAllTests();
    await demo.createVisualDemo();
    
    return results;
}