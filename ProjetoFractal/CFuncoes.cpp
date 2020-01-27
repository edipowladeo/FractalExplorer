
#include "Cfuncoes.h"


//OBSOLETO???


 unsigned int funcoes_mandelbrot::iteracoes_normalizada_int(double XX, double YY,int imax)
{
//    double distancia;
    double parcial;
    double log_zn;

    double zx,zy,zx2,zy2;//,xt;
    int i;
    zx=XX;
    zy=YY;
    zx2=zx*zx;
    zy2=zy*zy;
    for (i=0; (zx2+zy2 < LIMITE_DIVERGENCIA); i++)
    {
        zy*=zx;
        zy+=zy;
        zy+=YY;
        zx=zx2-zy2+XX;
        zx2=zx*zx;
        zy2=zy*zy;
        if (i==imax)
            return 0;
    }
    log_zn = log(zx2+zy2)/2;
    parcial = SAMPLING_ITERACOES*log(log_zn/0.69314718056)/0.69314718056; //parcela que falta para chegar no proximo i
    return i*SAMPLING_ITERACOES - parcial;
}

 float funcoes_mandelbrot::iteracoes_normalizada_float(double XX, double YY,int imax)
{
//    double distancia;
    double parcial;
    double log_zn;

    double zx,zy,zx2,zy2;//,xt;
    int i;
    zx=XX;
    zy=YY;
    zx2=zx*zx;
    zy2=zy*zy;
    for (i=0; (zx2+zy2 < LIMITE_DIVERGENCIA); i++)
    {
        zy*=zx;
     /*   if (abs(zy) > max_float)
        {
            max_float = zy;
            flag_float = 1;
        }*/
        zy+=zy;
       /* if (abs(zy) > max_float)
        {
            max_float = zy;
            flag_float = 1;
        }*/
        zy+=YY;
      /*  if (abs(zy) > max_float)
        {
            max_float = zy;
            flag_float = 1;
        }*/
        zx=zx2-zy2;
     /*   if (abs(zx) > max_float)
        {
            max_float = zx;
            flag_float = 2;
        }*/
        zx+=XX;
      /*  if (abs(zx) > max_float)
        {
            max_float = zx;
            flag_float = 2;
        }*/
        zx2=zx*zx;
      /*  if (abs(zx2) > max_float)
        {
            max_float = zx2;
            flag_float = 3;
        }*/
        zy2=zy*zy;
     /*   if (abs(zy2) > max_float)
        {
            max_float = zy2;
            flag_float = 4;
        }*/

        if (i==imax)
            return 0;
    }
    log_zn = log(zx2+zy2)/2;
    parcial = log(log_zn/0.69314718056)/0.69314718056; //parcela que falta para chegar no proximo i
    return i - parcial;
}



