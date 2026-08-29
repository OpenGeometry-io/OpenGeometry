// scripts/generate-layers.ts
import * as fs from 'fs';
import * as yaml from 'yaml';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

interface LayerMapping {
  layers: Record<string, string[]>;
}

function generateTypeScript() {
  const yamlPath = path.join(__dirname, '..', 'layers.yaml');
  const yamlContent = fs.readFileSync(yamlPath, 'utf-8');
  const mapping: LayerMapping = yaml.parse(yamlContent);

  let output = '// Auto-generated from layers.yaml\n';
  output += '// DO NOT EDIT MANUALLY\n\n';

  // Генерируем enum
  output += 'export enum AIALayer {\n';
  const sortedLayers = Object.keys(mapping.layers).sort();
  for (const layer of sortedLayers) {
    const key = layer.replace(/-/g, '_').toUpperCase();
    output += `  ${key} = '${layer}',\n`;
  }
  output += '}\n\n';

  // Генерируем маппинг
  output += 'export const ENTITY_TO_LAYER: Record<string, AIALayer> = {\n';
  const entries: string[] = [];
  for (const [layer, kinds] of Object.entries(mapping.layers)) {
    const enumKey = layer.replace(/-/g, '_').toUpperCase();
    for (const kind of kinds) {
      entries.push(`  '${kind.toLowerCase()}': AIALayer.${enumKey},`);
    }
  }
  entries.sort();
  output += entries.join('\n');
  output += '\n};\n\n';

  output += 'export function getLayerForEntity(kind: string): AIALayer | null {\n';
  output += '  const normalized = kind.toLowerCase().trim();\n';
  output += '  return ENTITY_TO_LAYER[normalized] || null;\n';
  output += '}\n';

  const outPath = path.join(
    __dirname,
    '..',
    'main/opengeometry-three/src/registry/layers.ts'
  );
  
  // Создаем директорию если не существует
  const dir = path.dirname(outPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  
  fs.writeFileSync(outPath, output);
  console.log(`✅ Generated ${outPath}`);
}

try {
  generateTypeScript();
} catch (error) {
  console.error('❌ Failed to generate layers:', error);
  process.exit(1);
}
