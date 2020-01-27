#ifndef CFUNCOES_H
#define CFUNCOES_H

#include <math.h>
#include "global_headers.h"


class funcoes_mandelbrot{
    public:
static unsigned int iteracoes_normalizada_int(double XX, double YY,int imax);
static float iteracoes_normalizada_float(double XX, double YY,int imax);
};


#endif
