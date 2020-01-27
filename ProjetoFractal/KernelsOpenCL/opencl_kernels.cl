      __kernel void itnorm1(  __global const float *XX,
				        __global const float *YY,
				        __global const float *imax,
				        __global       float *it)
 {
     int gid = get_global_id(0);

  //  float distancia;
  //  float parcial;
  //  float log_zn;
    float zx,zy,zx2,zy2,cx,cy,log_zn,parcial;
    float result = 0;
    int i;
    cx = XX[gid];
    cy = YY[gid];
    zx = zy = zx2=zy2=0;
    for (i=0;i<imax[gid]; i++)
    {
        zy*=zx;
        zy+=zy;
        zy+=cy;
        zx=zx2-zy2+cx;
        zx2=zx*zx;
        zy2=zy*zy;
    if (zx2+zy2>128){
    log_zn = log(zx2+zy2)/2;
       result = i- log(log_zn/0.69314718056)/0.69314718056; //parcela que falta para chegar no proximo i
     break;
    }
    }
  it[gid] = result;
  }


  __kernel void itn_1(  __global const float *XX,
				        __global const float *YY,
				        __global const float *imax,
				        __global       float *it)
 {
     int gid = get_global_id(0);

  //  float distancia;
  //  float parcial;
  //  float log_zn;
    float zx,zy,zx2,zy2,xt,cx,cy;
    int i;
    cx = XX[gid];
    cy = YY[gid];
    for (i=0;i<imax[gid]; i++)
    {
        zx2 = zx*zx;
        zy2 = zy*zy;
        if (zx2+zy2>32) break;
        xt = 2*zx*zy;
        zx = zx2 - zy2 + cx;
        zy = xt + cy;
    }
  it[gid] = i;}


   __kernel void itn_2(  __global const float *XX,
				        __global const float *YY,
				        __global const float *imax,
				        __global       float *it)
 {
     int gid = get_global_id(0);

  //  float distancia;
  //  float parcial;
  //  float log_zn;
    float zx,zy,zx2,zy2,xt,cx,cy;
    int i;
    cx = XX[gid];
    cy = YY[gid];
    zx = zy = zx2=zy2=0;
    for (i=0;i<imax[gid]; i++)
    {
        zy*=zx;
        zy+=zy;
        zy+=cy;
        zx=zx2-zy2+cx;
        zx2=zx*zx;
        zy2=zy*zy;
    if (zx2+zy2>32) break;
    }
  it[gid] = i;}

     __kernel void bench1( __global       float *it)
 {
     int gid = get_global_id(0);

  //  float distancia;
  //  float parcial;
  //  float log_zn;
    float zx,zy,zx2,zy2,xt,cx,cy;
    int i;
    cx = 0.000215;
    cy = 0.0554;
    zx = zy = zx2=zy2=0;
    for (i=0;i<100000; i++)
    {
        zy*=zx;
        zy+=zy;
        zy+=cy;
        zx=zx2-zy2+cx;
        zx2=zx*zx;
        zy2=zy*zy;
    if (zx2+zy2>32) break;
    }
    it[gid] = i;
    }




   /*
    zx=zy=0;
    for (i=0;i<5000; i++)
    {
        zx2 = zx*zx;
        zy2 = zy*zy;
        if (zx2+zy2 > 32) break;
        tx = 2*zx*zy;
        x = x2 - y2 + cx;
        y = tx + yc;
    }
    it[gid] = i +50 ;
 }

   /* log_zn = log(zx2+zy2)/2;
    parcial = RESOLUCAO_CORES*log(log_zn/0.69314718056)/0.69314718056; //parcela que falta para chegar no proximo i
    return i*RESOLUCAO_CORES - parcial;


     c[gid]  = a[gid] - b[gid]+2;
     d[gid]  = a[gid] * b[gid]+15000;*/
 //}

/*unsigned long int iteracoes_normalizada(double XX, double YY)
{
    double distancia;
    double parcial;
    double log_zn;

    double zx,zy,zx2,zy2,xt;
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
    parcial = RESOLUCAO_CORES*log(log_zn/0.69314718056)/0.69314718056; //parcela que falta para chegar no proximo i
    return i*RESOLUCAO_CORES - parcial;
}
*/

__kernel void vadd(  __global const float *a,
				     __global const float *b,
				     __global       float *c,
				     __global       float *d)
 {
     int gid = get_global_id(0);
     d[gid]  = a[gid] + b[gid];
     //d[gid]  = gid;
 }

  __kernel void vadd3(  __global const float *a,
				        __global const float *b,
				        __global const float *c,
                        __global       float *d)
 {
     int gid = get_global_id(0);
   //  c[gid]  = a[gid] - b[gid]+2;
     d[gid]  = a[gid] + b[gid]+100;
 }

  
__kernel void vector_add(__global const float *A, __global const float *B, __global float *C) {
 
    // Get the index of the current element to be processed
    int i = get_global_id(0);
 
    // Do the operation
    C[i] = A[i] + B[i];
}