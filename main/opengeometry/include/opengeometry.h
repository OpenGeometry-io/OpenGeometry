#ifndef OPENGEOMETRY_H
#define OPENGEOMETRY_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

// Forward declarations
typedef struct OGRectangleHandle OGRectangleHandle;
typedef struct OGMeshOwned OGMeshOwned;

// Rectangle primitive functions
OGRectangleHandle* og_rectangle_create(const char* id);
void og_rectangle_destroy(OGRectangleHandle* rect);
void og_rectangle_set_config(OGRectangleHandle* rect, 
                            double center_x, double center_y, double center_z,
                            double width, double height);
void og_rectangle_generate_geometry(OGRectangleHandle* rect);

// Mesh conversion functions  
OGMeshOwned* og_rectangle_to_mesh(OGRectangleHandle* rect);
void og_mesh_destroy(OGMeshOwned* mesh);

// Mesh data access functions for Vulkan
void og_mesh_get_vertices(const OGMeshOwned* mesh, const float** data, size_t* count);
void og_mesh_get_indices(const OGMeshOwned* mesh, const uint32_t** data, size_t* count);
void og_mesh_get_normals(const OGMeshOwned* mesh, const float** data, size_t* count);

// Utility functions
const char* og_get_version(void);
void og_free_string(char* str);

#ifdef __cplusplus
}
#endif

#endif // OPENGEOMETRY_H