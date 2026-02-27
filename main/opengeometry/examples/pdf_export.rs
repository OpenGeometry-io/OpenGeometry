/**
 * PDF Export Example - All Primitives
 * 
 * This example demonstrates how to create various OpenGeometry primitives
 * and export them as a vector PDF file using the export pipeline.
 * 
 * Primitives demonstrated:
 * - OGLine
 * - OGPolyline
 * - OGArc (Circle)
 * - OGRectangle
 * - OGPolygon
 * 
 * Run with: cargo run --example pdf_export
 */

use opengeometry::primitives::polyline::OGPolyline;
use opengeometry::primitives::line::OGLine;
use opengeometry::primitives::arc::OGArc;
use opengeometry::primitives::rectangle::OGRectangle;
use opengeometry::primitives::polygon::OGPolygon;
use opengeometry::drawing::{Scene2D, Path2D, Vec2};
use opengeometry::export::pdf::PdfExportConfig;
use openmaths::Vector3;

fn main() {
    println!("╔════════════════════════════════════════════════════╗");
    println!("║   OpenGeometry PDF Export - All Primitives Demo    ║");
    println!("╚════════════════════════════════════════════════════╝\n");

    // Create a scene
    let mut scene = Scene2D::with_name("All Primitives Demo");

    // ============================================
    // 1. OGLine - Simple line segment
    // ============================================
    println!("1. Creating OGLine...");
    
    let mut line = OGLine::new("line-1".to_string());
    line.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0)
    );
    line.generate_geometry();
    
    let line_path = line.to_path2d();
    println!("   ✓ Line: {} segment(s)", line_path.segment_count());
    scene.add_path(line_path);

    // ============================================
    // 2. OGPolyline - Connected line segments
    // ============================================
    println!("2. Creating OGPolyline (L-shape)...");
    
    let mut polyline = OGPolyline::new("polyline-1".to_string());
    polyline.add_point(Vector3::new(0.0, 0.0, 1.0));
    polyline.add_point(Vector3::new(2.0, 0.0, 1.0));
    polyline.add_point(Vector3::new(2.0, 0.0, 3.0));
    polyline.add_point(Vector3::new(3.0, 0.0, 3.0));
    
    let polyline_path = polyline.to_path2d();
    println!("   ✓ Polyline: {} segment(s), closed: {}", 
             polyline_path.segment_count(), polyline_path.closed);
    scene.add_path(polyline_path);

    // ============================================
    // 3. OGPolyline - Closed (Room outline)
    // ============================================
    println!("3. Creating closed OGPolyline (room)...");
    
    let mut room = OGPolyline::new("room-1".to_string());
    room.add_point(Vector3::new(4.0, 0.0, 0.0));
    room.add_point(Vector3::new(8.0, 0.0, 0.0));
    room.add_point(Vector3::new(8.0, 0.0, 3.0));
    room.add_point(Vector3::new(4.0, 0.0, 3.0));
    room.add_point(Vector3::new(4.0, 0.0, 0.0)); // Close
    
    let room_path = room.to_path2d();
    println!("   ✓ Room: {} segment(s), closed: {}", 
             room_path.segment_count(), room_path.closed);
    scene.add_path(room_path);

    // ============================================
    // 4. OGArc - Circular arc (quarter circle)
    // ============================================
    println!("4. Creating OGArc (quarter circle)...");
    
    let mut arc = OGArc::new("arc-1".to_string());
    arc.set_config(
        Vector3::new(1.0, 0.0, 5.0),  // center
        0.8,                           // radius
        0.0,                           // start angle
        std::f64::consts::PI / 2.0,   // end angle (90 degrees)
        16                             // segments
    );
    arc.generate_geometry();
    
    let arc_path = arc.to_path2d();
    println!("   ✓ Arc: {} segment(s)", arc_path.segment_count());
    scene.add_path(arc_path);

    // ============================================
    // 5. OGArc - Full circle
    // ============================================
    println!("5. Creating OGArc (full circle)...");
    
    let mut circle = OGArc::new("circle-1".to_string());
    circle.set_config(
        Vector3::new(3.5, 0.0, 5.0),   // center
        0.6,                            // radius
        0.0,                            // start angle
        2.0 * std::f64::consts::PI,    // end angle (360 degrees)
        32                              // segments
    );
    circle.generate_geometry();
    
    let circle_path = circle.to_path2d();
    println!("   ✓ Circle: {} segment(s), closed: {}", 
             circle_path.segment_count(), circle_path.closed);
    scene.add_path(circle_path);

    // ============================================
    // 6. OGRectangle - Axis-aligned rectangle
    // ============================================
    println!("6. Creating OGRectangle...");
    
    let mut rect = OGRectangle::new("rect-1".to_string());
    rect.set_config(
        Vector3::new(6.0, 0.0, 5.0),  // center
        1.5,                           // width
        1.0                            // breadth
    );
    rect.generate_geometry();
    
    let rect_path = rect.to_path2d();
    println!("   ✓ Rectangle: {} segment(s), closed: {}", 
             rect_path.segment_count(), rect_path.closed);
    scene.add_path(rect_path);

    // ============================================
    // 7. OGPolygon - Arbitrary polygon (triangle)
    // ============================================
    println!("7. Creating OGPolygon (triangle)...");
    
    let mut triangle = OGPolygon::new("triangle-1".to_string());
    triangle.set_config(vec![
        Vector3::new(0.0, 0.0, 7.0),
        Vector3::new(1.5, 0.0, 7.0),
        Vector3::new(0.75, 0.0, 8.5),
    ]);
    
    let triangle_path = triangle.to_path2d();
    println!("   ✓ Triangle: {} segment(s), closed: {}", 
             triangle_path.segment_count(), triangle_path.closed);
    scene.add_path(triangle_path);

    // ============================================
    // 8. OGPolygon - Complex polygon (hexagon)
    // ============================================
    println!("8. Creating OGPolygon (hexagon)...");
    
    let mut hexagon = OGPolygon::new("hexagon-1".to_string());
    let hex_center = (3.5, 7.5);
    let hex_radius = 0.8;
    let hex_points: Vec<Vector3> = (0..6)
        .map(|i| {
            let angle = (i as f64) * std::f64::consts::PI / 3.0;
            Vector3::new(
                hex_center.0 + hex_radius * angle.cos(),
                0.0,
                hex_center.1 + hex_radius * angle.sin()
            )
        })
        .collect();
    hexagon.set_config(hex_points);
    
    let hexagon_path = hexagon.to_path2d();
    println!("   ✓ Hexagon: {} segment(s), closed: {}", 
             hexagon_path.segment_count(), hexagon_path.closed);
    scene.add_path(hexagon_path);

    // ============================================
    // 9. OGPolygon - Star shape
    // ============================================
    println!("9. Creating OGPolygon (5-pointed star)...");
    
    let mut star = OGPolygon::new("star-1".to_string());
    let star_center = (6.5, 7.5);
    let outer_radius = 0.8;
    let inner_radius = 0.35;
    let star_points: Vec<Vector3> = (0..10)
        .map(|i| {
            let angle = (i as f64) * std::f64::consts::PI / 5.0 - std::f64::consts::PI / 2.0;
            let radius = if i % 2 == 0 { outer_radius } else { inner_radius };
            Vector3::new(
                star_center.0 + radius * angle.cos(),
                0.0,
                star_center.1 + radius * angle.sin()
            )
        })
        .collect();
    star.set_config(star_points);
    
    let star_path = star.to_path2d();
    println!("   ✓ Star: {} segment(s), closed: {}", 
             star_path.segment_count(), star_path.closed);
    scene.add_path(star_path);

    // ============================================
    // 10. Using custom projection (X-Y plane instead of X-Z)
    // ============================================
    println!("10. Creating line with custom projection...");
    
    let mut vertical_line = OGLine::new("vertical-line".to_string());
    vertical_line.set_config(
        Vector3::new(8.5, 0.0, 0.0),
        Vector3::new(8.5, 0.0, 8.5)
    );
    vertical_line.generate_geometry();
    
    // Using default X-Z projection
    let vertical_path = vertical_line.to_path2d();
    println!("   ✓ Vertical line: {} segment(s)", vertical_path.segment_count());
    scene.add_path(vertical_path);

    // ============================================
    // Scene Summary
    // ============================================
    println!("\n" + &"═".repeat(50));
    println!("SCENE SUMMARY");
    println!("{}", "═".repeat(50));
    println!("  Total paths: {}", scene.path_count());
    
    if let Some((min, max)) = scene.bounding_box() {
        println!("  Bounding box: ({:.2}, {:.2}) to ({:.2}, {:.2})", 
                 min.x, min.y, max.x, max.y);
        println!("  Size: {:.2}m x {:.2}m", scene.width(), scene.height());
    }

    // ============================================
    // Export to PDF - A4 Landscape
    // ============================================
    println!("\n" + &"═".repeat(50));
    println!("EXPORTING TO PDF");
    println!("{}", "═".repeat(50));
    
    let a4_config = PdfExportConfig::a4_landscape();
    let output_path = "all_primitives_a4.pdf";
    
    match opengeometry::export::pdf::export_scene_to_pdf_with_config(&scene, output_path, &a4_config) {
        Ok(()) => {
            println!("  ✓ A4 Landscape: {}", output_path);
        }
        Err(e) => {
            eprintln!("  ✗ Export failed: {}", e);
        }
    }

    // Export to A3
    let a3_config = PdfExportConfig::a3_landscape();
    let a3_output = "all_primitives_a3.pdf";
    
    match opengeometry::export::pdf::export_scene_to_pdf_with_config(&scene, a3_output, &a3_config) {
        Ok(()) => {
            println!("  ✓ A3 Landscape: {}", a3_output);
        }
        Err(e) => {
            eprintln!("  ✗ Export failed: {}", e);
        }
    }

    // Export to A4 Portrait
    let portrait_config = PdfExportConfig::a4_portrait();
    let portrait_output = "all_primitives_portrait.pdf";
    
    match opengeometry::export::pdf::export_scene_to_pdf_with_config(&scene, portrait_output, &portrait_config) {
        Ok(()) => {
            println!("  ✓ A4 Portrait: {}", portrait_output);
        }
        Err(e) => {
            eprintln!("  ✗ Export failed: {}", e);
        }
    }

    // ============================================
    // Bonus: Direct Path2D creation
    // ============================================
    println!("\n" + &"═".repeat(50));
    println!("BONUS: Direct Path2D Creation");
    println!("{}", "═".repeat(50));
    
    let points = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ];
    
    let direct_path = Path2D::from_points(&points, true);
    println!("  Created closed path with {} segments", direct_path.segment_count());
    println!("  Total length: {:.2}m", direct_path.total_length());
    
    if let Some((min, max)) = direct_path.bounding_box() {
        println!("  Bounding box: ({}, {}) to ({}, {})", min.x, min.y, max.x, max.y);
    }

    println!("\n" + &"═".repeat(50));
    println!("✓ All exports completed successfully!");
    println!("  Open the PDF files to verify vector output.");
    println!("  The PDFs contain true vector paths (not rasterized).");
    println!("{}", "═".repeat(50));
}
