/**
 * Тесты для OGEntityRegistry
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { OGEntityRegistry } from '../src/registry/index.js';
import type { ViewDescription, EntityDescription } from '../src/registry/index.js';

// Мокаем WASM-модуль
vi.mock('opengeometry', () => ({
  OGEntityRegistry: vi.fn().mockImplementation(() => ({
    registerEntity: vi.fn(),
    unregisterEntity: vi.fn().mockReturnValue(true),
    clearEntities: vi.fn(),
    projectCurrentToViews: vi.fn().mockReturnValue(
      JSON.stringify({
        'plan': {
          name: 'plan',
          segments: [
            {
              geometry: { type: 'Line', start: { x: 0, y: 0 }, end: { x: 1, y: 1 } },
              class: 'VisibleOutline',
              layer: 'A-WALL',
              sourceEntityId: 'wall-1'
            }
          ]
        }
      })
    )
  }))
}));

describe('OGEntityRegistry', () => {
  let registry: OGEntityRegistry;

  beforeEach(() => {
    registry = new OGEntityRegistry();
  });

  describe('registerEntity', () => {
    it('should register entity without errors', () => {
      expect(() => {
        registry.registerEntity('test-1', 'wall', '{"vertices":[]}');
      }).not.toThrow();
    });
  });

  describe('unregisterEntity', () => {
    it('should return true when entity exists', () => {
      const result = registry.unregisterEntity('test-1');
      expect(result).toBe(true);
    });

    it('should return false when entity does not exist', () => {
      // Мокаем возврат false для несуществующей сущности
      const mockUnregister = vi.fn().mockReturnValue(false);
      const registry2 = new OGEntityRegistry();
      (registry2 as any).wasmRegistry.unregisterEntity = mockUnregister;
      
      const result = registry2.unregisterEntity('nonexistent');
      expect(result).toBe(false);
    });
  });

  describe('clearEntities', () => {
    it('should clear all entities without errors', () => {
      expect(() => {
        registry.clearEntities();
      }).not.toThrow();
    });
  });

  describe('projectViews', () => {
    it('should project multiple views', () => {
      const views: ViewDescription[] = [
        {
          id: 'plan',
          camera: {
            position: [0, 10, 0],
            target: [0, 0, 0],
            up: [0, 0, -1],
            near: 0.01,
            projectionMode: 'Orthographic'
          }
        },
        {
          id: 'elevation-front',
          camera: {
            position: [0, 0, 10],
            target: [0, 0, 0],
            up: [0, 1, 0],
            near: 0.01,
            projectionMode: 'Orthographic'
          }
        }
      ];

      const result = registry.projectViews(views);
      expect(result).toHaveProperty('plan');
      expect(result).toHaveProperty('elevation-front');
    });
  });

  describe('projectStandardViews', () => {
    it('should generate standard views', () => {
      const entities: EntityDescription[] = [
        { id: 'wall-1', kind: 'wall', brepJson: '{}' },
        { id: 'door-1', kind: 'door', brepJson: '{}' }
      ];

      const result = registry.projectStandardViews(entities);
      
      // Проверяем наличие стандартных видов
      expect(result).toHaveProperty('plan');
      expect(result).toHaveProperty('elevation-front');
      expect(result).toHaveProperty('elevation-back');
      expect(result).toHaveProperty('elevation-left');
      expect(result).toHaveProperty('elevation-right');
      expect(result).toHaveProperty('isometric');
    });

    it('should respect options', () => {
      const entities: EntityDescription[] = [
        { id: 'wall-1', kind: 'wall', brepJson: '{}' }
      ];

      const result = registry.projectStandardViews(entities, {
        includePlan: false,
        includeElevations: false,
        includeIsometric: false,
        includeSections: true
      });

      // План отключен
      expect(result).not.toHaveProperty('plan');
      
      // Фасады отключены
      expect(result).not.toHaveProperty('elevation-front');
      
      // Изометрия отключена
      expect(result).not.toHaveProperty('isometric');
      
      // Разрезы включены
      expect(result).toHaveProperty('section-horizontal');
      expect(result).toHaveProperty('section-vertical-x');
    });

    it('should use custom target', () => {
      const entities: EntityDescription[] = [
        { id: 'wall-1', kind: 'wall', brepJson: '{}' }
      ];

      const customTarget: [number, number, number] = [5, 2, -3];
      
      // Проверяем, что функция не падает с кастомным target
      expect(() => {
        registry.projectStandardViews(entities, {
          target: customTarget
        });
      }).not.toThrow();
    });
  });

  describe('error handling', () => {
    it('should handle invalid JSON in BRep', () => {
      const registry2 = new OGEntityRegistry();
      
      // Мокаем ошибку WASM
      const mockRegister = vi.fn().mockImplementation(() => {
        throw new Error('Invalid BRep');
      });
      (registry2 as any).wasmRegistry.registerEntity = mockRegister;

      expect(() => {
        registry2.registerEntity('test', 'wall', 'invalid json');
      }).toThrow();
    });
  });
});
