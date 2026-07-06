// main/opengeometry-three/src/registry/index.ts
import { OGEntityRegistry as WasmRegistry } from 'opengeometry';

/**
 * Описание камеры для проекции
 */
export interface CameraDescription {
  /** Позиция камеры в 3D пространстве [x, y, z] */
  position: [number, number, number];
  /** Точка, на которую смотрит камера [x, y, z] */
  target: [number, number, number];
  /** Вектор "вверх" [x, y, z] */
  up: [number, number, number];
  /** Ближняя плоскость отсечения */
  near: number;
  /** Режим проекции: ортографическая или перспективная */
  projectionMode: 'Orthographic' | 'Perspective';
}

/**
 * Опции скрытия линий (HLR - Hidden Line Removal)
 */
export interface HlrOptions {
  /** Скрывать ли скрытые ребра */
  hideHiddenEdges: boolean;
}

/**
 * Плоскость сечения для разрезов
 */
export interface SectionPlane {
  /** Точка на плоскости [x, y, z] */
  origin: [number, number, number];
  /** Нормаль к плоскости [x, y, z] */
  normal: [number, number, number];
}

/**
 * Описание одного видового окна для проекции
 */
export interface ViewDescription {
  /** Уникальный идентификатор вида */
  id: string;
  /** Параметры камеры */
  camera: CameraDescription;
  /** Опции скрытия линий (опционально) */
  hlr?: HlrOptions;
  /** Плоскость сечения для разрезов (опционально) */
  sectionPlane?: SectionPlane;
}

/**
 * Геометрический сегмент в 2D проекции
 */
export type Segment2D =
  | { type: 'Line'; start: { x: number; y: number }; end: { x: number; y: number } }
  | { type: 'Arc'; center: { x: number; y: number }; radius: number; startAngle: number; endAngle: number }
  | { type: 'Ellipse'; center: { x: number; y: number }; rx: number; ry: number; rotation: number; startAngle: number; endAngle: number }
  | { type: 'CubicBezier'; p0: { x: number; y: number }; p1: { x: number; y: number }; p2: { x: number; y: number }; p3: { x: number; y: number } };

/**
 * Классифицированный сегмент с атрибуцией
 */
export interface ClassifiedSegment {
  /** Геометрия сегмента */
  geometry: Segment2D;
  /** Класс ребра по ISO 128 */
  class: 'VisibleOutline' | 'VisibleCrease' | 'VisibleSmooth' | 'Hidden' | 'SectionCut';
  /** AIA/NCS слой (например "A-WALL") */
  layer: string | null;
  /** ID исходной сущности в BRep */
  sourceEntityId: string | null;
}

/**
 * Результат проекции для одного вида
 */
export interface ViewResult {
  /** Имя вида */
  name: string | null;
  /** Массив классифицированных сегментов */
  segments: ClassifiedSegment[];
}

/**
 * Результат пакетной проекции - карта "ID вида -> результат"
 */
export type ProjectionResult = Record<string, ViewResult>;

/**
 * Описание сущности для регистрации
 */
export interface EntityDescription {
  /** Уникальный идентификатор сущности */
  id: string;
  /** Тип сущности (для определения слоя) */
  kind: string;
  /** BRep в формате JSON (сериализованный) */
  brepJson: string;
}

/**
 * Опции для проекции стандартных видов
 */
export interface StandardViewsOptions {
  /** Расстояние камеры от центра (по умолчанию 10) */
  orthographicDistance?: number;
  /** Включать ли вид сверху (план) */
  includePlan?: boolean;
  /** Включать ли фасады (4 стороны) */
  includeElevations?: boolean;
  /** Включать ли 3D-изометрию */
  includeIsometric?: boolean;
  /** Центр сцены для ориентации камер */
  target?: [number, number, number];
}

/**
 * Реестр сущностей для пакетной многовидовой проекции.
 * 
 * @example
 * ```typescript
 * const registry = new OGEntityRegistry();
 * 
 * // Регистрация сущностей
 * registry.registerEntity('wall-1', 'wall', wallBrepJson);
 * registry.registerEntity('door-1', 'door', doorBrepJson);
 * 
 * // Создание видов
 * const views = registry.projectViews([
 *   {
 *     id: 'plan',
 *     camera: {
 *       position: [0, 10, 0],
 *       target: [0, 0, 0],
 *       up: [0, 0, -1],
 *       near: 0.01,
 *       projectionMode: 'Orthographic'
 *     },
 *     hlr: { hideHiddenEdges: true }
 *   },
 *   {
 *     id: 'elevation-front',
 *     camera: {
 *       position: [0, 0, 10],
 *       target: [0, 0, 0],
 *       up: [0, 1, 0],
 *       near: 0.01,
 *       projectionMode: 'Orthographic'
 *     }
 *   }
 * ]);
 * 
 * // Получение сегментов с атрибуцией
 * const planSegments = views['plan'].segments;
 * for (const seg of planSegments) {
 *   console.log(`Layer: ${seg.layer}, Source: ${seg.sourceEntityId}`);
 * }
 * ```
 */
export class OGEntityRegistry {
  private wasmRegistry: WasmRegistry;

  constructor() {
    this.wasmRegistry = new WasmRegistry();
  }

  /**
   * Зарегистрировать (или заменить) сущность в реестре
   * kind будет нормализован (приведен к lowercase) в Rust
   */
  registerEntity(id: string, kind: string, brepJson: string): void {
    this.wasmRegistry.registerEntity(id, kind, brepJson);
  }

  /**
   * Удалить сущность из реестра
   */
  unregisterEntity(id: string): boolean {
    return this.wasmRegistry.unregisterEntity(id);
  }

  /**
   * Очистить все сущности из реестра
   */
  clearEntities(): void {
    this.wasmRegistry.clearEntities();
  }

  /**
   * Выполнить пакетную проекцию для нескольких видов за один вызов WASM
   */
  projectViews(views: ViewDescription[]): ProjectionResult {
    const viewsJson = JSON.stringify(views);
    const resultJson = this.wasmRegistry.projectCurrentToViews(viewsJson);
    return JSON.parse(resultJson);
  }

  /**
   * Создать временный реестр и выполнить проекцию стандартных видов
   * 
   * @param entities - Массив сущностей для регистрации
   * @param options - Опции генерации видов
   * @returns Результаты проекции
   * 
   * @note Этот метод не изменяет текущий реестр, он создает временный
   */
  static projectStandardViews(
    entities: EntityDescription[],
    options: StandardViewsOptions = {}
  ): ProjectionResult {
    // Создаем временный реестр
    const tempRegistry = new OGEntityRegistry();
    
    // Регистрируем все сущности
    for (const entity of entities) {
      tempRegistry.registerEntity(entity.id, entity.kind, entity.brepJson);
    }

    const views: ViewDescription[] = [];
    const dist = options.orthographicDistance ?? 10;
    const target: [number, number, number] = options.target ?? [0, 0, 0];

    // План (вид сверху)
    if (options.includePlan !== false) {
      views.push({
        id: 'plan',
        camera: {
          position: [0, dist, 0],
          target: target,
          up: [0, 0, -1],
          near: 0.01,
          projectionMode: 'Orthographic',
        },
        hlr: { hideHiddenEdges: true },
      });
    }

    // Фасады (4 стороны)
    if (options.includeElevations !== false) {
      const elevations: Array<{ id: string; pos: [number, number, number] }> = [
        { id: 'elevation-front', pos: [0, 0, dist] },
        { id: 'elevation-back', pos: [0, 0, -dist] },
        { id: 'elevation-left', pos: [-dist, 0, 0] },
        { id: 'elevation-right', pos: [dist, 0, 0] },
      ];
      for (const elev of elevations) {
        views.push({
          id: elev.id,
          camera: {
            position: elev.pos,
            target: target,
            up: [0, 1, 0],
            near: 0.01,
            projectionMode: 'Orthographic',
          },
          hlr: { hideHiddenEdges: true },
        });
      }
    }

    // Изометрия
    if (options.includeIsometric !== false) {
      views.push({
        id: 'isometric',
        camera: {
          position: [dist, dist * 0.7, dist] as [number, number, number],
          target: target,
          up: [0, 1, 0],
          near: 0.01,
          projectionMode: 'Orthographic',
        },
        hlr: { hideHiddenEdges: true },
      });
    }

    return tempRegistry.projectViews(views);
  }
}
